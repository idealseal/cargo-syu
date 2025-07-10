use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};

/// cargo-like help style messages.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

/// This is a wrapper around the cargo CLI. When calling cargo syu, cargo will invoke this binary
/// with 'syu' as the first argument. This wrapper will swallow the command.
#[derive(clap::Parser)]
#[command(bin_name = "cargo")]
#[command(about = None)]
#[command(styles = STYLES)]
#[command(after_help = "You meant to invoke the `syu` subcommand.")]
#[command(version)]
pub(crate) enum Cli {
    Syu(SyuArgs),
}

/// Update Rust binary crates.
#[derive(clap::Args)]
pub(crate) struct SyuArgs {
    /// Ask before installing packages.
    #[arg(short, long)]
    pub(crate) ask: bool,

    /// Include packages installed with --git.
    #[arg(short, long)]
    pub(crate) git: bool,

    /// Print installed packages and available updates but don't try to update them.
    #[arg(short, long, group = "output", visible_alias = "dry-run")]
    pub(crate) list: bool,

    #[command(flatten)]
    pub(crate) package_args: PackageArgs,

    #[command(flatten)]
    pub(crate) install_args: InstallArgs,
}

#[derive(clap::Args)]
#[command(next_help_heading = "Package Selection")]
pub(crate) struct PackageArgs {
    /// Comma separated list of packages to exclude.
    #[arg(short, long, name = "PACKAGE", value_delimiter = ',')]
    pub(crate) exclude: Option<Vec<String>>,
}

#[derive(clap::Args)]
#[command(next_help_heading = "Installation Options")]
pub(crate) struct InstallArgs {
    /// Number of parallel jobs, defaults to # of CPUs.
    #[arg(short, long, name = "N")]
    pub(crate) jobs: Option<u8>,

    /// Don't pass --locked to cargo install.
    ///
    /// cargo-syu will use --locked by default to install packages to minimize breakage.
    #[arg(long)]
    pub(crate) no_locked: bool,

    /// Use verbose output.
    #[arg(short, long)]
    pub(crate) verbose: bool,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
