use anyhow::{Result, Context, bail};
use crate::command_utils::execute_command;

pub(crate) struct Config {
    pub(crate) source_vm: Option<String>,
}

impl Config {
    pub(crate) fn load() -> Result<Self> {
        let source_vm = get_git_config("qsync.source-vm")?;
        
        Ok(Config {
            source_vm,
        })
    }
    
    pub(crate) fn get_source_vm(&self) -> Result<String> {
        self.source_vm
            .clone()
            .context("Configuration missing: qsync.source-vm")
    }
}

fn get_git_config(key: &str) -> Result<Option<String>> {
    let output = execute_command("git", &["config", "--get", key])?;
    
    if output.status.success() {
        let value = String::from_utf8(output.stdout)?
            .trim()
            .to_string();
        Ok(if value.is_empty() { None } else { Some(value) })
    } else {
        Ok(None)
    }
}

pub(crate) fn get_project_name() -> Result<String> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("Cannot determine project name from current directory")?;
    
    Ok(project_name.to_string())
}

pub(crate) fn get_current_branch() -> Result<String> {
    let output = execute_command("git", &["rev-parse", "--abbrev-ref", "HEAD"])?;
    
    if !output.status.success() {
        bail!("Not in a git repository");
    }
    
    let branch = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
    
    Ok(branch)
}

pub(crate) fn check_git_repo() -> Result<()> {
    let output = execute_command("git", &["rev-parse", "--git-dir"])?;
    
    if !output.status.success() {
        bail!("Not in a git repository");
    }
    
    Ok(())
}