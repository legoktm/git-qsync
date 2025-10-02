use anyhow::Result;
use camino::Utf8Path as Path;
use clap::{Parser, Subcommand};

pub(crate) mod command_utils;
pub(crate) mod config;
pub(crate) mod export;
pub(crate) mod import;
pub(crate) mod system_config;

use crate::command_utils::execute_command;
use crate::system_config::SystemConfig;

#[derive(Parser)]
#[command(name = "git-qsync")]
#[command(about = "Transfer git branches between Qubes VMs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "e")]
    Export {
        /// Branch to export (defaults to current branch)
        branch: Option<String>,
    },
    #[command(alias = "i")]
    Import {
        /// Specific bundle file to import
        bundle_file: Option<String>,
    },
    /// Set up git aliases for qe and qi shortcuts
    Init,
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let system_config = SystemConfig::from_env();

    match cli.command {
        Commands::Export { branch } => {
            export::run(branch, &system_config)?;
        }
        Commands::Import { bundle_file } => {
            import::run(bundle_file)?;
        }
        Commands::Init => {
            setup_git_aliases()?;
        }
    }

    Ok(())
}

fn setup_git_aliases() -> Result<()> {
    println!("Setting up git aliases for qsync shortcuts...");

    // Set up git qe alias
    let qe_alias = "!git qsync export";
    execute_command(
        "git",
        &["config", "--global", "alias.qe", qe_alias],
        Path::new("."),
    )?;
    println!("✓ Set up 'git qe' alias");

    // Set up git qi alias
    let qi_alias = "!git qsync import";
    execute_command(
        "git",
        &["config", "--global", "alias.qi", qi_alias],
        Path::new("."),
    )?;
    println!("✓ Set up 'git qi' alias");

    println!();
    println!("Git aliases configured successfully!");
    println!("You can now use:");
    println!("  git qe [branch]    # Export branch (same as git qsync export)");
    println!("  git qi [bundle]    # Import bundle (same as git qsync import)");

    Ok(())
}
