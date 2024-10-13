use std::path::{Path, PathBuf};

use cargo_syu::{read_installed_packages, LocalPackage, Package};
use clap::{
    builder::{
        styling::{AnsiColor, Effects},
        Styles,
    },
    Parser,
};
use owo_colors::OwoColorize;

/// `cargo`-like clap style.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(clap::Parser)]
#[command(name = "cargo", bin_name = "cargo", styles = STYLES)]
enum CargoCli {
    Syu(SyuArgs),
}

/// `pacman -Syu`, just for cargo.
#[derive(clap::Args)]
#[command(version)]
struct SyuArgs {
    /// List packages without updating them
    #[arg(short, long, visible_alias = "dry-run")]
    list: bool,

    /// Include git packages
    #[arg(short, long)]
    git: bool,
}

/// A structure representing a cargo config file.
#[derive(serde::Deserialize)]
struct CargoConfig {
    install: Option<InstallConfig>,
}

/// A structure representing an install table within a cargo config file.
#[derive(serde::Deserialize)]
struct InstallConfig {
    /// Cargo installation root
    root: Option<PathBuf>,
}

/// Read a file as a cargo config file and try to read it's `[install.root]` property.
// TODO: Chaining options of is easier, but preserving the result here might help to notify about
// the exact type of error in this function
pub fn read_install_root<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    std::fs::read_to_string(path)
        .ok()
        // TODO: Inform about parsing error
        .and_then(|data| toml::from_str::<CargoConfig>(&data).ok())
        .and_then(|config| config.install)
        .and_then(|install| install.root)
}

/// Determine the path of `.crates.toml`.
fn get_crates_files() -> anyhow::Result<PathBuf> {
    let cargo_root = home::cargo_home()?;
    let install_root = read_install_root(cargo_root.join("config.toml"))
        // TODO: Handle absolute and current relative paths
        .and_then(|root| home::home_dir().map(|home| home.join(root)))
        .unwrap_or(cargo_root);
    Ok(install_root.join(".crates.toml"))
}

fn main() -> anyhow::Result<()> {
    let CargoCli::Syu(args) = CargoCli::parse();

    let crates_file = get_crates_files()?;
    if !crates_file.exists() {
        return Ok(());
    }

    let packages = read_installed_packages(crates_file)?
        .into_iter()
        .filter(|pkg| args.git || matches!(pkg, LocalPackage::Registry { .. }))
        .filter_map(|pkg| pkg.fetch().ok())
        .collect::<Vec<_>>();

    let len = std::cmp::max(
        packages
            .iter()
            .map(|pkg| match pkg {
                Package::Registry { name, .. } | Package::Git { name, .. } => name.len(),
            })
            .max()
            .unwrap_or(7),
        7, // Length of the string `Package`
    );
    println!("{len}");
    if packages.is_empty() {
        return Ok(());
    }

    println!(
        "{:>12} {:<len$} {:>9} {:>9}",
        "Status".bold().green(),
        "Package",
        "Installed",
        "Available"
    );
    packages.iter().for_each(|pkg| pkg.print_package(len));

    if !args.list {
        for pkg in packages {
            pkg.update()?;
        }
    }

    Ok(())
}
