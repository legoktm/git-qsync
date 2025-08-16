use anyhow::Result;
use crate::error::QSyncError;
use crate::command_utils::execute_command;

pub struct Config {
    pub source_vm: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let source_vm = get_git_config("qsync.source-vm")?;
        
        Ok(Config {
            source_vm,
        })
    }
    
    pub fn get_source_vm(&self) -> Result<String> {
        self.source_vm
            .clone()
            .ok_or_else(|| QSyncError::ConfigMissing { 
                key: "qsync.source-vm".to_string() 
            }.into())
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

pub fn get_project_name() -> Result<String> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| QSyncError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Cannot determine project name from current directory"
        )))?;
    
    Ok(project_name.to_string())
}

pub fn get_current_branch() -> Result<String> {
    let output = execute_command("git", &["rev-parse", "--abbrev-ref", "HEAD"])?;
    
    if !output.status.success() {
        return Err(QSyncError::NotInGitRepo.into());
    }
    
    let branch = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
    
    Ok(branch)
}

pub fn check_git_repo() -> Result<()> {
    let output = execute_command("git", &["rev-parse", "--git-dir"])?;
    
    if !output.status.success() {
        return Err(QSyncError::NotInGitRepo.into());
    }
    
    Ok(())
}