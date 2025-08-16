use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::Path;

mod config;
mod export;
mod import;
mod command_utils;

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
    env_logger::init();
    
    // Check if called as git-qe or git-qi shortcuts
    let binary_path = std::env::args().next().unwrap_or_default();
    let binary_name = Path::new(&binary_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("git-qsync");
    
    match binary_name {
        "git-qe" => {
            // Parse args manually for git-qe [branch]
            let args: Vec<String> = std::env::args().collect();
            let branch = if args.len() > 1 { Some(args[1].clone()) } else { None };
            export::run(branch)?;
        }
        "git-qi" => {
            // Parse args manually for git-qi [bundle-file]
            let args: Vec<String> = std::env::args().collect();
            let bundle_file = if args.len() > 1 { Some(args[1].clone()) } else { None };
            import::run(bundle_file)?;
        }
        _ => {
            // Normal CLI parsing for git-qsync
            let cli = Cli::parse();
            
            match cli.command {
                Commands::Export { branch } => {
                    export::run(branch)?;
                }
                Commands::Import { bundle_file } => {
                    import::run(bundle_file)?;
                }
            }
        }
    }
    
    Ok(())
}
