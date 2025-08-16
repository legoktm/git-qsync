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

