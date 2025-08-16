use anyhow::Result;
use clap::Parser;

mod config;
mod export;
mod error;

#[derive(Parser)]
#[command(name = "git-qe")]
#[command(about = "Export git branch to Qubes VM (shortcut for git qsync export)")]
struct Cli {
    /// Branch to export (defaults to current branch)
    branch: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    export::run(cli.branch)?;
    Ok(())
}