use crate::command_utils::execute_command;
use crate::config::{check_git_repo, get_project_name, Config};
use anyhow::{bail, Context, Result};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use dialoguer::{Confirm, Select};
use std::fs;

pub(crate) fn run(bundle_file: Option<String>) -> Result<()> {
    check_git_repo()?;

    let config = Config::load()?;
    let source_vm = config.get_source_vm()?;
    let project_name = get_project_name()?;

    let bundle_path = match bundle_file {
        Some(file) => PathBuf::from(file),
        None => {
            let qubes_incoming_path = format!(
                "{}/QubesIncoming/{}/git-qsync/{}",
                std::env::var("HOME")?,
                source_vm,
                project_name
            );
            find_latest_bundle(&qubes_incoming_path)?
        }
    };

    println!("Found bundle: {}", bundle_path);

    // Verify bundle
    verify_bundle(&bundle_path)?;

    // Extract branch name from bundle
    let branch_name = extract_branch_name(&bundle_path)?;
    println!("Bundle contains branch: {}", branch_name);

    // Check if branch exists locally
    let branch_exists = check_branch_exists(&branch_name)?;

    let final_branch_name = if branch_exists {
        handle_branch_conflict(&branch_name)?
    } else {
        branch_name.clone()
    };

    // Handle branch overwriting if needed
    if branch_exists && final_branch_name == branch_name {
        delete_branch_safely(&final_branch_name)?;
    }

    // Import the bundle
    import_bundle(&bundle_path, &branch_name, &final_branch_name)?;

    println!(
        "Successfully imported branch '{}' as '{}'",
        branch_name, final_branch_name
    );

    // Switch to the imported branch
    switch_to_branch(&final_branch_name)?;

    Ok(())
}

fn find_latest_bundle(dir_path: &str) -> Result<PathBuf> {
    let path = Path::new(dir_path);

    if !path.exists() {
        bail!("No bundle files found in {}", dir_path);
    }

    let mut bundles = Vec::new();

    for entry in path.read_dir_utf8()? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let entry_path = entry.path();
            if let Some(ext) = entry_path.extension() {
                if ext == "bundle" {
                    bundles.push(entry_path.to_path_buf());
                }
            }
        }
    }

    if bundles.is_empty() {
        bail!("No bundle files found in {}", dir_path);
    }

    // Sort by modification time (newest first)
    bundles.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH)
    });
    bundles.reverse();

    Ok(bundles[0].clone())
}

fn verify_bundle(bundle_path: &Path) -> Result<()> {
    let output = execute_command("git", &["bundle", "verify", bundle_path.as_str()])?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        eprintln!("Bundle verification failed:");
        eprintln!("{}", error_msg);
        bail!("Bundle verification failed");
    }

    println!("Bundle verification successful");
    Ok(())
}

fn extract_branch_name(bundle_path: &Path) -> Result<String> {
    let output = execute_command("git", &["bundle", "list-heads", bundle_path.as_str()])?;

    if !output.status.success() {
        bail!("Git command failed: Failed to list bundle heads");
    }

    let output_str = String::from_utf8(output.stdout)?;
    let first_line = output_str
        .lines()
        .next()
        .context("Git command failed: Bundle contains no refs")?;

    // Parse line like: "abc123... refs/heads/feature-branch"
    let branch_ref = first_line
        .split_whitespace()
        .nth(1)
        .context("Git command failed: Invalid bundle head format")?;

    let branch_name = branch_ref.strip_prefix("refs/heads/").unwrap_or(branch_ref);

    Ok(branch_name.to_string())
}

fn check_branch_exists(branch_name: &str) -> Result<bool> {
    check_branch_exists_at_path(".", branch_name)
}

fn check_branch_exists_at_path(path: &str, branch_name: &str) -> Result<bool> {
    let repo = gix::open(path)?;
    let reference_name = format!("refs/heads/{}", branch_name);

    match repo.refs.find(&reference_name) {
        Ok(_) => Ok(true),
        Err(gix::refs::file::find::existing::Error::NotFound { .. }) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn get_current_branch_at_path(path: &str) -> Result<String> {
    let repo = gix::open(path)?;
    let head = repo.head()?;

    match head.referent_name() {
        Some(name) => {
            // Convert from refs/heads/branch-name to branch-name
            let name_str = name.as_bstr().to_string();
            if let Some(branch_name) = name_str.strip_prefix("refs/heads/") {
                Ok(branch_name.to_string())
            } else {
                // If it's not a branch reference, return the full name
                Ok(name_str)
            }
        }
        None => {
            if head.is_detached() {
                bail!("HEAD is detached, not on a branch");
            } else {
                bail!("HEAD is unborn or in an unexpected state");
            }
        }
    }
}

fn delete_branch_safely(branch_name: &str) -> Result<()> {
    delete_branch_safely_at_path(".", branch_name)
}

fn delete_branch_safely_at_path(repo_path: &str, branch_name: &str) -> Result<()> {
    let is_current = is_branch_checked_out_at_path(repo_path, branch_name)?;

    if is_current {
        // Switch to a safe branch before deleting
        // Try to switch to main, then master, then create a temporary branch
        let safe_branches = ["main", "master"];
        let mut switched = false;

        for safe_branch in &safe_branches {
            if check_branch_exists_at_path(repo_path, safe_branch)? && safe_branch != &branch_name {
                let output = execute_command_at_path("git", &["checkout", safe_branch], repo_path)?;
                if output.status.success() {
                    println!(
                        "Switched to '{}' before deleting '{}'",
                        safe_branch, branch_name
                    );
                    switched = true;
                    break;
                }
            }
        }

        if !switched {
            // Create a temporary branch from HEAD~1 or from the first commit
            let temp_branch = format!("temp-before-import-{}", branch_name);
            let output = execute_command_at_path(
                "git",
                &["checkout", "-b", &temp_branch, "HEAD~1"],
                repo_path,
            )?;
            if !output.status.success() {
                // Try from first commit if HEAD~1 doesn't work
                let output = execute_command_at_path(
                    "git",
                    &["checkout", "--orphan", &temp_branch],
                    repo_path,
                )?;
                if !output.status.success() {
                    bail!(
                        "Cannot switch away from branch '{}' to delete it",
                        branch_name
                    );
                }
                // Clear the index for orphan branch
                let _ = execute_command_at_path("git", &["reset", "--hard"], repo_path);
            }
            println!(
                "Created temporary branch '{}' before deleting '{}'",
                temp_branch, branch_name
            );
        }
    }

    // Now delete the branch
    let output = execute_command_at_path("git", &["branch", "-D", branch_name], repo_path)?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to delete branch '{}': {}", branch_name, error_msg);
    }

    println!("Deleted existing branch '{}'", branch_name);
    Ok(())
}

fn switch_to_branch(branch_name: &str) -> Result<()> {
    switch_to_branch_at_path(".", branch_name)
}

fn switch_to_branch_at_path(repo_path: &str, branch_name: &str) -> Result<()> {
    let output = execute_command_at_path("git", &["checkout", branch_name], repo_path)?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Failed to switch to branch '{}': {}",
            branch_name,
            error_msg
        );
    }

    println!("Switched to branch '{}'", branch_name);
    Ok(())
}

fn is_branch_checked_out_at_path(repo_path: &str, branch_name: &str) -> Result<bool> {
    let current_branch = get_current_branch_at_path(repo_path)?;
    Ok(current_branch == branch_name)
}

fn execute_command_at_path(
    cmd: &str,
    args: &[&str],
    repo_path: &str,
) -> Result<std::process::Output> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .current_dir(repo_path)
        .output()?;
    Ok(output)
}

fn handle_branch_conflict(branch_name: &str) -> Result<String> {
    println!("Branch '{}' already exists. Choose action:", branch_name);

    let options = vec![
        "Overwrite existing branch (destructive)",
        "Import as new branch name",
        "Cancel import",
    ];

    let selection = Select::new()
        .with_prompt("Choose action")
        .items(&options)
        .default(1)
        .interact()?;

    match selection {
        0 => {
            let confirm = Confirm::new()
                .with_prompt("This will permanently overwrite the existing branch. Continue?")
                .default(false)
                .interact()?;

            if confirm {
                Ok(branch_name.to_string())
            } else {
                bail!("Cancelled by user")
            }
        }
        1 => {
            let new_name = format!("import-{}", branch_name);
            println!("Importing as '{}'", new_name);
            Ok(new_name)
        }
        2 => bail!("Cancelled by user"),
        _ => unreachable!(),
    }
}

fn import_bundle(bundle_path: &Path, original_branch: &str, target_branch: &str) -> Result<()> {
    let refspec = format!(
        "refs/heads/{}:refs/heads/{}",
        original_branch, target_branch
    );

    let output = execute_command("git", &["fetch", bundle_path.as_str(), &refspec])?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Git command failed: Failed to import bundle: {}", error_msg);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn setup_test_git_repo(dir: &std::path::Path) -> Result<()> {
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .context("Failed to init git repo")?;

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir)
            .output()
            .context("Failed to set git user.name")?;

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir)
            .output()
            .context("Failed to set git user.email")?;

        std::fs::write(dir.join("README.md"), "# Test Repo")?;
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(dir)
            .output()
            .context("Failed to add file")?;

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir)
            .output()
            .context("Failed to commit")?;

        Ok(())
    }

    fn create_branch(dir: &std::path::Path, branch_name: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .current_dir(dir)
            .output()
            .context("Failed to create branch")?;

        if !output.status.success() {
            bail!(
                "Failed to create branch: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Switch back to main to leave the branch available for testing
        let output = Command::new("git")
            .args(["checkout", "main"])
            .current_dir(dir)
            .output();

        let switched = match output {
            Ok(output) if output.status.success() => true,
            _ => {
                // Fallback to master if main doesn't exist
                let output = Command::new("git")
                    .args(["checkout", "master"])
                    .current_dir(dir)
                    .output();
                matches!(output, Ok(output) if output.status.success())
            }
        };

        if !switched {
            bail!(
                "Failed to switch back to main/master branch after creating {}",
                branch_name
            );
        }

        Ok(())
    }

    #[test]
    fn test_get_current_branch_on_main() {
        let temp_dir = TempDir::new().unwrap();

        // Setup test directory
        setup_test_git_repo(temp_dir.path()).unwrap();

        let result = get_current_branch_at_path(temp_dir.path().to_str().unwrap());

        assert!(
            result.is_ok(),
            "Failed to get current branch: {:?}",
            result.err()
        );
        let branch = result.unwrap();
        assert!(
            branch == "main" || branch == "master",
            "Expected main or master, got: {}",
            branch
        );
    }

    #[test]
    fn test_get_current_branch_on_feature_branch() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();

        // Create and checkout a feature branch
        let output = Command::new("git")
            .args(["checkout", "-b", "feature-test"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "Failed to create feature branch: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let result = get_current_branch_at_path(temp_dir.path().to_str().unwrap());

        assert!(
            result.is_ok(),
            "Failed to get current branch: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), "feature-test");
    }

    #[test]
    fn test_get_current_branch_detached_head() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();

        // Get the commit hash and checkout directly to it (detached HEAD)
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed to get HEAD commit");
        let commit_hash = String::from_utf8(output.stdout).unwrap().trim().to_string();

        let output = Command::new("git")
            .args(["checkout", &commit_hash])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed to checkout detached HEAD");

        let result = get_current_branch_at_path(temp_dir.path().to_str().unwrap());

        assert!(result.is_err(), "Should fail on detached HEAD");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("HEAD is detached") || error_msg.contains("unborn"),
            "Unexpected error: {}",
            error_msg
        );
    }

    #[test]
    fn test_check_branch_exists_true() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();
        create_branch(temp_dir.path(), "existing-branch").unwrap();

        let result =
            check_branch_exists_at_path(temp_dir.path().to_str().unwrap(), "existing-branch");

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_check_branch_exists_false() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();

        let result =
            check_branch_exists_at_path(temp_dir.path().to_str().unwrap(), "non-existent-branch");

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_is_branch_checked_out() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();
        create_branch(temp_dir.path(), "test-branch").unwrap();

        // Check that main/master is currently checked out
        let current = get_current_branch_at_path(temp_dir.path().to_str().unwrap()).unwrap();

        let result = is_branch_checked_out_at_path(temp_dir.path().to_str().unwrap(), &current);
        assert!(
            result.is_ok(),
            "Failed to check if branch is checked out: {:?}",
            result.err()
        );
        assert!(result.unwrap());

        // Check that test-branch is not checked out
        let result =
            is_branch_checked_out_at_path(temp_dir.path().to_str().unwrap(), "test-branch");
        assert!(
            result.is_ok(),
            "Failed to check if branch is checked out: {:?}",
            result.err()
        );
        assert!(!result.unwrap());
    }

    #[test]
    fn test_delete_branch_safely_not_current() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();
        create_branch(temp_dir.path(), "delete-me").unwrap();

        // Verify branch exists before deletion using the path-based function
        assert!(
            check_branch_exists_at_path(temp_dir.path().to_str().unwrap(), "delete-me").unwrap(),
            "Branch should exist before deletion"
        );

        // Verify we're on a different branch (not the one we're about to delete)
        let current_branch = get_current_branch_at_path(temp_dir.path().to_str().unwrap()).unwrap();
        assert_ne!(
            current_branch, "delete-me",
            "Should not be on the branch we're deleting"
        );

        let result = delete_branch_safely_at_path(temp_dir.path().to_str().unwrap(), "delete-me");

        assert!(
            result.is_ok(),
            "Failed to delete branch: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_delete_branch_safely_currently_checked_out() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();

        // Create and checkout feature branch
        let output = Command::new("git")
            .args(["checkout", "-b", "feature-to-delete"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Failed to create branch: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify we're on the branch we want to delete
        let current_branch = get_current_branch_at_path(temp_dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            current_branch, "feature-to-delete",
            "Should be on the branch we're about to delete"
        );

        // Verify the branch exists before trying to delete it
        assert!(
            check_branch_exists_at_path(temp_dir.path().to_str().unwrap(), "feature-to-delete")
                .unwrap(),
            "Branch should exist before deletion"
        );

        // This should switch away and delete the branch
        let result =
            delete_branch_safely_at_path(temp_dir.path().to_str().unwrap(), "feature-to-delete");

        assert!(
            result.is_ok(),
            "Failed to delete branch: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_switch_to_branch() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();
        create_branch(temp_dir.path(), "switch-to-me").unwrap();

        let result = switch_to_branch_at_path(temp_dir.path().to_str().unwrap(), "switch-to-me");

        assert!(
            result.is_ok(),
            "Failed to switch to branch: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_switch_to_non_existent_branch() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();

        let result =
            switch_to_branch_at_path(temp_dir.path().to_str().unwrap(), "non-existent-branch");

        assert!(
            result.is_err(),
            "Should fail when switching to non-existent branch"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to switch to branch"));
    }
}
