//! Update Rust binary crates.
//!
//! ## Installation
//!
//! ```console
//! cargo install --locked cargo-syu
//! ```
//!
//! After that, the command can update itself.
//!
//! ## Usage
//!
//! The default operation is to list and update packages.
//!
//! ```console
//! cargo syu
//! ```
//!
//! Don't update packages, but list available updates.
//!
//! ```console
//! cargo syu --list
//! ```
//!
//! Take git packages into consideration.
//!
//! ```console
//! cargo syu --git
//! ```
//!
//! ## Development plan
//!
//! - [x] Find cargo install root from `~/.cargo/config.toml`.
//! - [x] List installed packages.
//! - [x] Update installed packages.
//! - [x] Find and update git packages.
//! - [ ] Detect registry URL from `.crates.toml`.
//! - [ ] Print progress bar for metadata fetching.
//! - [ ] Improved handling of errors
//!     1. [ ] Don't fail immediately if one package operation failed.
//!     2. [ ] Display warning for failed package, but continue for remaining packages.
//! - [ ] Allow more or less verbose output.
//! - [ ] Allow printing of outdated packages only.
//! - [ ] Add more code documentation.
//! - [ ] Write unit and integration tests.
//! - [x] Add --ask flag to require user confirmation before installing packages.

mod cli;

use core::str::FromStr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{bail, Context as _, Error, Result};
use clap::Parser as _;
use git2::{Direction, Repository};
use inquire::prompt_confirmation;
use owo_colors::OwoColorize as _;
use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};
use semver::Version;

use crate::cli::{Cli, InstallArgs, PackageArgs};

fn main() -> Result<()> {
    let Cli::Syu(args) = Cli::parse();

    let cargo_root = home::cargo_home()?;
    let cargo_root_config = cargo_root.join("config.toml");

    let cargo_install_root = if cargo_root_config.exists() {
        let config = std::fs::read_to_string(cargo_root_config)?;
        let config = toml::from_str::<CargoConfig>(&config)?;
        config.install.and_then(|install| install.root)
    } else {
        None
    };
    let cargo_install_root =
        cargo_install_root.map_or(cargo_root, |root| home::home_dir().unwrap().join(root));

    let crates_toml = cargo_install_root.join(".crates.toml");
    if !crates_toml.exists() {
        return Ok(());
    }

    let crates_toml = std::fs::read_to_string(crates_toml)?;
    let crates = toml::Value::from_str(&crates_toml)?;
    let crates = crates
        .get("v1")
        .and_then(|v1| v1.as_table())
        .context("Couldn't read crates from `.crates.toml`")?;

    let crates = crates
        .into_iter()
        .map(|(pkg, _)| Package::from_str(pkg))
        .collect::<Result<Vec<_>>>()?;

    // Filter package list based on upstream and command line arguments.
    let PackageArgs { exclude } = args.package_args;
    let crates = crates
        .into_iter()
        // Filter invalid packages and packages with a local source.
        .filter(|pkg| !matches!(pkg.upstream, Upstream::Unknown))
        // Filter packages which have been excluded from the command line.
        .filter(|pkg| {
            !exclude
                .as_ref()
                .is_some_and(|list| list.contains(&pkg.name))
        })
        // Remove git packages when --git was not specified.
        .filter(|pkg| args.git || matches!(pkg.upstream, Upstream::Registry { .. }))
        .collect::<Vec<_>>();

    let crates: Vec<LatestPackage> = crates
        .into_par_iter()
        .map(Package::fetch_latest_version)
        .collect::<Result<_>>()?;

    let len = crates
        .iter()
        .map(|pkg| pkg.name.len())
        .max()
        .unwrap_or(7)
        .max(7);
    println!(
        "{:>12} {:<len$} {:>9} {:>9}",
        "Status".bold().green(),
        "Package",
        "Installed",
        "Available"
    );
    crates.iter().for_each(|pkg| pkg.print(len));

    if args.list {
        return Ok(());
    }

    if !crates.is_empty() && args.ask && prompt_confirmation("Install packages?").unwrap_or(false) {
        return Ok(());
    }

    let InstallArgs {
        jobs,
        no_locked,
        verbose,
    } = args.install_args;
    crates
        .into_iter()
        .try_for_each(|pkg| pkg.update(jobs, !no_locked, verbose))?;

    Ok(())
}

#[derive(serde::Deserialize)]
struct CargoConfig {
    install: Option<CargoInstallConfig>,
}

#[derive(serde::Deserialize)]
struct CargoInstallConfig {
    root: Option<PathBuf>,
}

struct Package {
    name: String,
    upstream: Upstream,
}

enum Upstream {
    Git { url: String, commit: String },
    Registry { version: Version },
    Unknown,
}

impl FromStr for Package {
    type Err = Error;

    fn from_str(value: &str) -> core::result::Result<Self, Self::Err> {
        let (name, remainder) = value
            .split_once(' ')
            .context(format!("Failed to split package name: {value}"))?;
        let (vers, upstream) = remainder
            .split_once(' ')
            .context(format!("Failed to split package version: {remainder}"))?;

        let upstream = upstream
            .strip_prefix('(')
            .context(format!("Missing enclosing '(' for {upstream}"))?;
        let upstream = upstream
            .strip_suffix(')')
            .context(format!("Missing enclosing ')' for {upstream}"))?;
        let (kind, url) = upstream
            .split_once('+')
            .context(format!("Failed to split package source: {upstream}"))?;

        let name = name.to_owned();
        let upstream = match kind {
            "git" => {
                let (url, commit) = url
                    .split_once('#')
                    .context(format!("Failed to split git commit: {url}"))?;
                let url = url.split_once('?').map_or(url, |s| s.0);

                let url = url.to_owned();
                let commit = commit.to_owned();
                Upstream::Git { url, commit }
            }
            "registry" | "sparse" => {
                let vers = Version::from_str(vers)?;
                Upstream::Registry { version: vers }
            }
            _ => Upstream::Unknown,
        };
        Ok(Self { name, upstream })
    }
}

impl Package {
    fn fetch_latest_version(self) -> Result<LatestPackage> {
        let name = self.name;
        let upstream = match self.upstream {
            Upstream::Git { url, commit } => {
                let dir = tempfile::tempdir()?;

                let repo = Repository::init_bare(dir)?;
                let mut remote = repo.remote_anonymous(&url)?;
                let conn = remote.connect_auth(Direction::Fetch, None, None)?;

                let latest_commit = conn
                    .list()?
                    .iter()
                    .next()
                    .context("Failed to get HEAD commit hash")
                    .map(|head| head.oid().to_string())?;

                LatestUpstream::Git {
                    url,
                    commit,
                    latest_commit,
                }
            }
            Upstream::Registry { version } => {
                let url = format!(
                    "https://index.crates.io/{}",
                    get_registry_package_path(&name)
                );
                let body = ureq::get(&url).call()?.into_string()?;

                let latest_entry = body
                    .lines()
                    .last()
                    .context(format!("Package index empty for {name}"))?;
                let latest_registry_version: RegistryVersion = serde_json::from_str(latest_entry)?;
                let latest_version = latest_registry_version.vers;

                LatestUpstream::Registry {
                    version,
                    latest_version,
                }
            }
            Upstream::Unknown => {
                bail!("Can't update package information for package of type `Unknown`")
            }
        };
        Ok(LatestPackage { name, upstream })
    }
}

fn get_registry_package_path(name: &str) -> String {
    assert!(
        !name.is_empty(),
        "Tried to get index package path for an empty package name"
    );
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{}", &name[..1], name),
        _ => format!("{}/{}/{}", &name[..2], &name[2..4], name),
    }
}

#[derive(serde::Deserialize)]
struct RegistryVersion {
    vers: Version,
}

struct LatestPackage {
    name: String,
    upstream: LatestUpstream,
}

enum LatestUpstream {
    Git {
        url: String,
        commit: String,
        latest_commit: String,
    },
    Registry {
        version: Version,
        latest_version: Version,
    },
}

impl LatestPackage {
    fn has_update(&self) -> bool {
        match &self.upstream {
            LatestUpstream::Git {
                commit,
                latest_commit,
                ..
            } => commit != latest_commit,
            LatestUpstream::Registry {
                version,
                latest_version,
                ..
            } => version < latest_version,
        }
    }

    fn print(&self, len: usize) {
        let update = self.has_update();
        let name = &self.name;
        let status = if update {
            "Update".style(*STATUS_UPDATE_STYLE)
        } else {
            "Current".style(*STATUS_CURRENT_STYLE)
        };

        match &self.upstream {
            LatestUpstream::Git {
                commit,
                latest_commit,
                ..
            } => {
                let latest_commit = latest_commit.style(if update {
                    *VERSION_UPDATE_STYLE
                } else {
                    *VERSION_CURRENT_STYLE
                });
                println!("{status:>12} {name:<len$} {commit:>9.9} {latest_commit:>9.9}");
            }
            LatestUpstream::Registry {
                version,
                latest_version,
                ..
            } => {
                let latest_version = latest_version.style(if update {
                    *VERSION_UPDATE_STYLE
                } else {
                    *VERSION_CURRENT_STYLE
                });
                println!("{status:>12} {name:<len$} {version:>9.9} {latest_version:>9.9}");
            }
        }
    }

    fn update(&self, jobs: Option<u8>, locked: bool, verbose: bool) -> Result<()> {
        if self.has_update() {
            update(self, jobs, locked, verbose)?;
        }
        Ok(())
    }
}

fn update(pkg: &LatestPackage, jobs: Option<u8>, locked: bool, verbose: bool) -> Result<()> {
    let mut command = Command::new("cargo");
    command.arg("install");

    if let Some(jobs) = jobs {
        command.arg("--jobs").arg(format!("{jobs}"));
    }
    if locked {
        command.arg("--locked");
    }
    if verbose {
        command.arg("--verbose");
    }

    match &pkg.upstream {
        LatestUpstream::Git { url, .. } => {
            command.arg("--git").arg(url).arg(&pkg.name);
        }
        LatestUpstream::Registry { .. } => {
            command.arg(&pkg.name);
        }
    }

    command.spawn()?.wait()?;
    Ok(())
}

static STATUS_CURRENT_STYLE: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bold().bright_black());

static STATUS_UPDATE_STYLE: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bold().green());

static VERSION_CURRENT_STYLE: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bright_black());

static VERSION_UPDATE_STYLE: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bold().cyan());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_registry_path() {
        assert_eq!(get_registry_package_path("c"), "1/c");
        assert_eq!(get_registry_package_path("ca"), "2/ca");
        assert_eq!(get_registry_package_path("car"), "3/c/car");
        assert_eq!(get_registry_package_path("cargo-syu"), "ca/rg/cargo-syu");
    }

    #[test]
    fn package_has_update() {
        assert!(LatestPackage {
            name: "".to_owned(),
            upstream: LatestUpstream::Registry {
                version: Version::new(1, 0, 0),
                latest_version: Version::new(1, 0, 1),
            },
        }
        .has_update());
        assert!(!LatestPackage {
            name: "".to_owned(),
            upstream: LatestUpstream::Registry {
                version: Version::new(1, 0, 0),
                latest_version: Version::new(1, 0, 0),
            },
        }
        .has_update());
        assert!(LatestPackage {
            name: "".to_owned(),
            upstream: LatestUpstream::Git {
                url: "".to_owned(),
                commit: "ccd28e7939cf3feed230944cfc3a0498b98bddab".to_owned(),
                latest_commit: "bb9f36d2fd022a089d39455d86d6c14e572628f1".to_owned()
            },
        }
        .has_update());
        assert!(!LatestPackage {
            name: "".to_owned(),
            upstream: LatestUpstream::Git {
                url: "".to_owned(),
                commit: "ccd28e7939cf3feed230944cfc3a0498b98bddab".to_owned(),
                latest_commit: "ccd28e7939cf3feed230944cfc3a0498b98bddab".to_owned()
            },
        }
        .has_update());
    }
}
