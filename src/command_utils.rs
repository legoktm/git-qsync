use std::process::{Command, Output};
use log::debug;
use anyhow::Result;

/// Execute a command with debug logging
pub fn execute_command(cmd: &str, args: &[&str]) -> Result<Output> {
    debug!("Executing command: {} {}", cmd, args.join(" "));
    
    let output = Command::new(cmd)
        .args(args)
        .output()?;
    
    if output.status.success() {
        debug!("Command succeeded: {} {}", cmd, args.join(" "));
        if !output.stdout.is_empty() {
            debug!("stdout: {}", String::from_utf8_lossy(&output.stdout).trim());
        }
        if !output.stderr.is_empty() {
            debug!("stderr: {}", String::from_utf8_lossy(&output.stderr).trim());
        }
    } else {
        debug!("Command failed: {} {} (exit code: {:?})", cmd, args.join(" "), output.status.code());
        if !output.stdout.is_empty() {
            debug!("stdout: {}", String::from_utf8_lossy(&output.stdout).trim());
        }
        if !output.stderr.is_empty() {
            debug!("stderr: {}", String::from_utf8_lossy(&output.stderr).trim());
        }
    }
    
    Ok(output)
}

/// Execute a command in a specific directory with debug logging
pub fn execute_command_in_dir(cmd: &str, args: &[&str], dir: &std::path::Path) -> Result<Output> {
    debug!("Executing command in {}: {} {}", dir.display(), cmd, args.join(" "));
    
    let output = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .output()?;
    
    if output.status.success() {
        debug!("Command succeeded in {}: {} {}", dir.display(), cmd, args.join(" "));
        if !output.stdout.is_empty() {
            debug!("stdout: {}", String::from_utf8_lossy(&output.stdout).trim());
        }
        if !output.stderr.is_empty() {
            debug!("stderr: {}", String::from_utf8_lossy(&output.stderr).trim());
        }
    } else {
        debug!("Command failed in {}: {} {} (exit code: {:?})", dir.display(), cmd, args.join(" "), output.status.code());
        if !output.stdout.is_empty() {
            debug!("stdout: {}", String::from_utf8_lossy(&output.stdout).trim());
        }
        if !output.stderr.is_empty() {
            debug!("stderr: {}", String::from_utf8_lossy(&output.stderr).trim());
        }
    }
    
    Ok(output)
}