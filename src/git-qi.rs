use anyhow::Result;
use clap::Parser;

mod config;
mod import;
mod error;

#[derive(Parser)]
#[command(name = "git-qi")]
#[command(about = "Import git branch from Qubes VM (shortcut for git qsync import)")]
struct Cli {
    /// Specific bundle file to import
    bundle_file: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    import::run(cli.bundle_file)?;
    Ok(())
}