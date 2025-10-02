use anyhow::Result;
use camino::Utf8Path as Path;
use log::debug;
use std::process::{Command, Output};

/// Execute a command with debug logging in a specific directory
pub(crate) fn execute_command(cmd: &str, args: &[&str], current_dir: &Path) -> Result<Output> {
    debug!("Executing command: {} {}", cmd, args.join(" "));

    let output = Command::new(cmd)
        .args(args)
        .current_dir(current_dir.as_std_path())
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
        debug!(
            "Command failed: {} {} (exit code: {:?})",
            cmd,
            args.join(" "),
            output.status.code()
        );
        if !output.stdout.is_empty() {
            debug!("stdout: {}", String::from_utf8_lossy(&output.stdout).trim());
        }
        if !output.stderr.is_empty() {
            debug!("stderr: {}", String::from_utf8_lossy(&output.stderr).trim());
        }
    }

    Ok(output)
}
