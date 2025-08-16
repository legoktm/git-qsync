use clap::{Parser, Subcommand};
use anyhow::Result;

mod config;
mod export;
mod import;
mod error;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Export { branch } => {
            export::run(branch)?;
        }
        Commands::Import { bundle_file } => {
            import::run(bundle_file)?;
        }
    }
    
    Ok(())
}
