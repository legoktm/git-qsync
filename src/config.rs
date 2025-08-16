use std::process::Command;
use anyhow::Result;
use crate::error::QSyncError;

pub struct Config {
    pub target_vm: Option<String>,
    pub source_vm: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let target_vm = get_git_config("qsync.target-vm")?;
        let source_vm = get_git_config("qsync.source-vm")?;
        
        Ok(Config {
            target_vm,
            source_vm,
        })
    }
    
    pub fn get_target_vm(&self) -> Result<String> {
        self.target_vm
            .clone()
            .ok_or_else(|| QSyncError::ConfigMissing { 
                key: "qsync.target-vm".to_string() 
            }.into())
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
    let output = Command::new("git")
        .args(["config", "--get", key])
        .output()?;
    
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
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    
    if !output.status.success() {
        return Err(QSyncError::NotInGitRepo.into());
    }
    
    let branch = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
    
    Ok(branch)
}

pub fn check_git_repo() -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()?;
    
    if !output.status.success() {
        return Err(QSyncError::NotInGitRepo.into());
    }
    
    Ok(())
}