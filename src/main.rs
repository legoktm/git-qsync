use clap::{Parser, Subcommand};
use anyhow::Result;

pub(crate) mod config;
pub(crate) mod export;
pub(crate) mod import;
pub(crate) mod command_utils;

use crate::command_utils::execute_command;

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
    
    match cli.command {
        Commands::Export { branch } => {
            export::run(branch)?;
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
    
    // Get the path to the current binary
    let current_exe = std::env::current_exe()?;
    let exe_path = current_exe.to_string_lossy();
    
    // Set up git qe alias
    let qe_alias = format!("!{} export", exe_path);
    execute_command("git", &["config", "--global", "alias.qe", &qe_alias])?;
    println!("✓ Set up 'git qe' alias");
    
    // Set up git qi alias  
    let qi_alias = format!("!{} import", exe_path);
    execute_command("git", &["config", "--global", "alias.qi", &qi_alias])?;
    println!("✓ Set up 'git qi' alias");
    
    println!();
    println!("Git aliases configured successfully!");
    println!("You can now use:");
    println!("  git qe [branch]    # Export branch (same as git qsync export)");
    println!("  git qi [bundle]    # Import bundle (same as git qsync import)");
    
    Ok(())
}
