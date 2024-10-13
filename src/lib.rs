use std::{path::Path, process::Command, str::FromStr, sync::LazyLock};

use anyhow::Context;
use owo_colors::OwoColorize;

static STATUS_CURRENT: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bold().bright_black());

static STATUS_UPDATE: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bold().green());

static VERSION_CURRENT: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bright_black());

static VERSION_UPDATE: LazyLock<owo_colors::Style> =
    LazyLock::new(|| owo_colors::Style::new().bold().cyan());

#[derive(Debug, PartialEq, Eq)]
pub enum LocalPackage {
    Git {
        name: String,
        vers: semver::Version,
        url: String,
        commit: String,
    },
    Registry {
        name: String,
        vers: semver::Version,
        url: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Package {
    Git {
        name: String,
        vers: semver::Version,
        url: String,
        commit: String,
        origin_commit: String,
    },
    Registry {
        name: String,
        vers: semver::Version,
        url: String,
        best_vers: semver::Version,
    },
}

impl FromStr for LocalPackage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, remainder) = s.split_once(' ').context("Failed to parse package name")?;
        let (vers, url) = remainder
            .split_once(' ')
            .context("Failed to parse package version")?;
        let vers = semver::Version::parse(vers)?;
        match url
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split_once('+')
        {
            Some((kind, url)) => match kind {
                // TODO: Parse branches, revision and tags
                "git" => {
                    let (url, commit) =
                        url.split_once('#').context("Missing commit hash in url")?;
                    let (url, _) = url.split_once('?').unwrap_or((url, ""));
                    Ok(Self::Git {
                        name: name.to_string(),
                        vers,
                        url: url.to_string(),
                        commit: commit.to_string(),
                    })
                }
                "registry" | "sparse" => Ok(Self::Registry {
                    name: name.to_string(),
                    vers,
                    url: url.to_string(),
                }),
                _ => anyhow::bail!("Invalid kind"),
            },
            None => anyhow::bail!("Failed to parse package url"),
        }
    }
}

impl LocalPackage {
    pub fn fetch(self) -> anyhow::Result<Package> {
        match self {
            Self::Git {
                name,
                vers,
                url,
                commit,
            } => fetch_git_commit(&url).map(|origin_commit| Package::Git {
                name,
                vers,
                url,
                commit,
                origin_commit,
            }),
            Self::Registry { name, vers, url } => {
                fetch_registry_version(&url, &name).map(|best_vers| Package::Registry {
                    name,
                    vers,
                    url,
                    best_vers,
                })
            }
        }
    }
}

fn fetch_git_commit(url: &str) -> anyhow::Result<String> {
    let dir = tempfile::tempdir()?;
    let repo = git2::Repository::init_bare(dir)?;
    let mut remote = repo.remote_anonymous(url)?;
    let conn = remote.connect_auth(git2::Direction::Fetch, None, None)?;
    conn.list()?
        .iter()
        .next()
        .context("Failed to get HEAD commit hash")
        .map(|head| head.oid().to_string())
}

/// A structure representing a single line in a package index.
#[derive(Debug, serde::Deserialize)]
struct IndexEntry {
    vers: semver::Version,
}

fn get_index_file(name: &str) -> String {
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

fn fetch_registry_version(url: &str, name: &str) -> anyhow::Result<semver::Version> {
    let url = if url == "https://github.com/rust-lang/crates.io-index" {
        "https://index.crates.io"
    } else {
        url
    };
    let url = format!("{url}/{}", get_index_file(name));
    let body = reqwest::blocking::get(url)?.text()?;
    let entry = body
        .lines()
        .last()
        .and_then(|line| serde_json::from_str::<IndexEntry>(line).ok())
        .context("Failed to parse index entry")?;
    Ok(entry.vers)
}

impl Package {
    #[must_use]
    pub fn has_update(&self) -> bool {
        match self {
            Self::Git {
                commit,
                origin_commit,
                ..
            } => commit != origin_commit,
            Self::Registry {
                vers, best_vers, ..
            } => vers < best_vers,
        }
    }

    pub fn update(&self) -> anyhow::Result<()> {
        if self.has_update() {
            match self {
                Self::Git { name, url, .. } => {
                    Command::new("cargo")
                        .arg("install")
                        .arg("--git")
                        .arg(url)
                        .arg(name)
                        .spawn()?
                        .wait()?;
                }
                Self::Registry { name, .. } => {
                    Command::new("cargo")
                        .arg("install")
                        .arg(name)
                        .spawn()?
                        .wait()?;
                }
            }
        }
        Ok(())
    }

    pub fn print_package(&self, len: usize) {
        let update = self.has_update();
        let status = if update {
            "Update".style(*STATUS_UPDATE)
        } else {
            "Current".style(*STATUS_CURRENT)
        };
        match self {
            Self::Git {
                name,
                commit,
                origin_commit,
                ..
            } => {
                let origin_commit = origin_commit.style(if update {
                    *VERSION_UPDATE
                } else {
                    *VERSION_CURRENT
                });
                println!(
                    "{:>12} {:<len$} {:>9.9} {:>9.9}",
                    status,
                    name.bright_black(),
                    commit.bright_black(),
                    origin_commit,
                );
            }
            Self::Registry {
                name,
                vers,
                best_vers,
                ..
            } => {
                let best_vers = best_vers.style(if update {
                    *VERSION_UPDATE
                } else {
                    *VERSION_CURRENT
                });
                println!(
                    "{:>12} {:<len$} {:>9.9} {:>9.9}",
                    status,
                    name.bright_black(),
                    vers.bright_black(),
                    best_vers,
                );
            }
        }
    }
}

fn list_installed_packages(data: &str) -> anyhow::Result<Vec<LocalPackage>> {
    let value = toml::Value::from_str(data)?;
    let v1 = value.get("v1").context("Key `v1` not found")?;
    let table = v1.as_table().context("Key `v1` is not a table")?;
    Ok(table
        .into_iter()
        // TODO: Notify about parsing errors
        .filter_map(|(key, _value)| LocalPackage::from_str(key).ok())
        .collect())
}

pub fn read_installed_packages<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<LocalPackage>> {
    let data = std::fs::read_to_string(path)?;
    list_installed_packages(&data)
}
