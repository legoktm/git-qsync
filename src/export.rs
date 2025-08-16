use std::process::Command;
use anyhow::Result;
use chrono::Utc;
use crate::config::{Config, check_git_repo, get_current_branch, get_project_name};
use crate::error::QSyncError;

pub fn run(branch: Option<String>) -> Result<()> {
    check_git_repo()?;
    
    let config = Config::load()?;
    let target_vm = config.get_target_vm()?;
    
    let branch_name = match branch {
        Some(b) => b,
        None => get_current_branch()?,
    };
    
    let project_name = get_project_name()?;
    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S");
    let bundle_filename = format!("{}_{}_{}.bundle", project_name, branch_name, timestamp);
    
    println!("Exporting branch '{}' to VM '{}'...", branch_name, target_vm);
    
    // Get default branch and merge base
    let default_branch = get_default_branch()?;
    let merge_base = get_merge_base(&branch_name, &default_branch)?;
    
    // Create bundle
    let bundle_range = format!("{}..{}", merge_base, branch_name);
    create_bundle(&bundle_filename, &bundle_range)?;
    
    // Move bundle to target VM
    move_bundle_to_vm(&bundle_filename, &target_vm)?;
    
    println!("Successfully exported branch '{}' to '{}'", branch_name, target_vm);
    
    Ok(())
}

fn get_default_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()?;
    
    if !output.status.success() {
        // Fallback to main/master (remote first, then local)
        let main_exists = Command::new("git")
            .args(["rev-parse", "--verify", "refs/remotes/origin/main"])
            .output()?
            .status
            .success();
        
        if main_exists {
            return Ok("origin/main".to_string());
        }
        
        let master_exists = Command::new("git")
            .args(["rev-parse", "--verify", "refs/remotes/origin/master"])
            .output()?
            .status
            .success();
        
        if master_exists {
            return Ok("origin/master".to_string());
        }
        
        // Try local branches if no remote
        let local_main_exists = Command::new("git")
            .args(["rev-parse", "--verify", "refs/heads/main"])
            .output()?
            .status
            .success();
        
        if local_main_exists {
            return Ok("main".to_string());
        }
        
        let local_master_exists = Command::new("git")
            .args(["rev-parse", "--verify", "refs/heads/master"])
            .output()?
            .status
            .success();
        
        if local_master_exists {
            return Ok("master".to_string());
        }
        
        return Err(QSyncError::GitCommandFailed {
            message: "Cannot determine default branch".to_string()
        }.into());
    }
    
    let default_ref = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
    
    // Extract branch name from refs/remotes/origin/main
    let branch = default_ref
        .strip_prefix("refs/remotes/")
        .unwrap_or(&default_ref);
    
    Ok(branch.to_string())
}

fn get_merge_base(branch: &str, default_branch: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["merge-base", branch, default_branch])
        .output()?;
    
    if !output.status.success() {
        return Err(QSyncError::GitCommandFailed {
            message: format!("Cannot find merge base between {} and {}", branch, default_branch)
        }.into());
    }
    
    let merge_base = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
    
    Ok(merge_base)
}

fn create_bundle(filename: &str, range: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["bundle", "create", filename, range])
        .output()?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(QSyncError::GitCommandFailed {
            message: format!("Bundle creation failed: {}", error_msg)
        }.into());
    }
    
    Ok(())
}

fn move_bundle_to_vm(filename: &str, target_vm: &str) -> Result<()> {
    let output = Command::new("qvm-move")
        .args([filename, target_vm])
        .output()?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(QSyncError::GitCommandFailed {
            message: format!("qvm-move failed: {}", error_msg)
        }.into());
    }
    
    Ok(())
}