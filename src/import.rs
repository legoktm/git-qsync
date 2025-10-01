use crate::command_utils::execute_command;
use crate::config::{check_git_repo, get_project_name, Config};
use anyhow::{bail, Context, Result};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use dialoguer::Select;
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

    // Open the repository once and reuse it
    let repo = gix::open(".")?;

    // Check if branch exists locally
    let branch_exists = check_branch_exists(&repo, &branch_name)?;

    let final_branch_name = if branch_exists {
        handle_branch_conflict(&branch_name)?
    } else {
        branch_name.clone()
    };

    // Handle branch overwriting if needed
    let temp_branch_created = if branch_exists && final_branch_name == branch_name {
        delete_branch_safely(&repo, &final_branch_name)?
    } else {
        None
    };

    // Import the bundle
    import_bundle(&bundle_path, &branch_name, &final_branch_name)?;

    println!(
        "Successfully imported branch '{}' as '{}'",
        branch_name, final_branch_name
    );

    // Switch to the imported branch
    switch_to_branch(&repo, &final_branch_name)?;

    // Clean up temporary branch if one was created
    if let Some(temp_branch) = temp_branch_created {
        cleanup_temp_branch(&repo, &temp_branch)?;
    }

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

fn check_branch_exists(repo: &gix::Repository, branch_name: &str) -> Result<bool> {
    let reference_name = format!("refs/heads/{}", branch_name);

    match repo.refs.find(&reference_name) {
        Ok(_) => Ok(true),
        Err(gix::refs::file::find::existing::Error::NotFound { .. }) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn get_default_branch(repo: &gix::Repository) -> Result<Option<String>> {
    // Try to get the default branch from the remote HEAD reference
    if let Ok(Some(remote_head_ref)) = repo.try_find_reference("refs/remotes/origin/HEAD") {
        if let gix::refs::TargetRef::Symbolic(name) = remote_head_ref.target() {
            let full_name = name.as_bstr().to_string();
            if let Some(branch_name) = full_name.strip_prefix("refs/remotes/origin/") {
                return Ok(Some(branch_name.to_string()));
            }
        }
    }

    Ok(None)
}

fn get_current_branch(repo: &gix::Repository) -> Result<String> {
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

fn delete_branch_safely(repo: &gix::Repository, branch_name: &str) -> Result<Option<String>> {
    let is_current = is_branch_checked_out(repo, branch_name)?;
    let repo_path = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))
        .and_then(|p| {
            Path::from_path(p).ok_or_else(|| anyhow::anyhow!("Repository path is not valid UTF-8"))
        })?;

    let temp_branch_name = if is_current {
        // Switch to a safe branch before deleting
        let mut switched = false;

        // First, try to switch to the detected default branch
        if let Ok(Some(default_branch)) = get_default_branch(repo) {
            if check_branch_exists(repo, &default_branch)? && default_branch != branch_name {
                let output =
                    execute_command_at_path("git", &["checkout", &default_branch], repo_path)?;
                if output.status.success() {
                    println!(
                        "Switched to default branch '{}' before deleting '{}'",
                        default_branch, branch_name
                    );
                    switched = true;
                }
            }
        }

        if !switched {
            // Try to switch to any other existing local branch
            if let Ok(iter) = repo.references()?.local_branches() {
                for branch_ref in iter.flatten() {
                    if let Some(name) = branch_ref.name().category_and_short_name() {
                        let other_branch = name.1.to_string();
                        if other_branch != branch_name && !other_branch.is_empty() {
                            let output = execute_command_at_path(
                                "git",
                                &["checkout", &other_branch],
                                repo_path,
                            )?;
                            if output.status.success() {
                                println!(
                                    "Switched to existing branch '{}' before deleting '{}'",
                                    other_branch, branch_name
                                );
                                switched = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !switched {
                // Create a temporary branch from HEAD~1 or from the first commit
                let temp_branch = format!("temp-before-import-{}", branch_name);

                // Delete the temp branch if it already exists (from a previous failed import)
                if check_branch_exists(repo, &temp_branch)? {
                    let output =
                        execute_command_at_path("git", &["branch", "-D", &temp_branch], repo_path)?;
                    if !output.status.success() {
                        let error_msg = String::from_utf8_lossy(&output.stderr);
                        bail!(
                            "Failed to delete existing temporary branch '{}': {}",
                            temp_branch,
                            error_msg
                        );
                    }
                }

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
                            "Cannot switch away from branch '{}' to delete it. Git errors:\nHEAD~1 checkout: {}\nOrphan checkout: {}",
                            branch_name,
                            String::from_utf8_lossy(&output.stderr),
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    // Clear the index for orphan branch
                    let _ = execute_command_at_path("git", &["reset", "--hard"], repo_path);
                }
                println!(
                    "Created temporary branch '{}' before deleting '{}'",
                    temp_branch, branch_name
                );
                Some(temp_branch)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Now delete the branch
    let output = execute_command_at_path("git", &["branch", "-D", branch_name], repo_path)?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to delete branch '{}': {}", branch_name, error_msg);
    }

    println!("Deleted existing branch '{}'", branch_name);
    Ok(temp_branch_name)
}

fn switch_to_branch(repo: &gix::Repository, branch_name: &str) -> Result<()> {
    let repo_path = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))
        .and_then(|p| {
            Path::from_path(p).ok_or_else(|| anyhow::anyhow!("Repository path is not valid UTF-8"))
        })?;
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

fn cleanup_temp_branch(repo: &gix::Repository, temp_branch_name: &str) -> Result<()> {
    let repo_path = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))
        .and_then(|p| {
            Path::from_path(p).ok_or_else(|| anyhow::anyhow!("Repository path is not valid UTF-8"))
        })?;

    // Check if the temporary branch still exists
    if check_branch_exists(repo, temp_branch_name)? {
        let output =
            execute_command_at_path("git", &["branch", "-D", temp_branch_name], repo_path)?;

        if output.status.success() {
            println!("Cleaned up temporary branch '{}'", temp_branch_name);
        } else {
            // Don't fail the entire import if cleanup fails - just warn
            let error_msg = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "Warning: Failed to clean up temporary branch '{}': {}",
                temp_branch_name, error_msg
            );
        }
    }

    Ok(())
}

fn is_branch_checked_out(repo: &gix::Repository, branch_name: &str) -> Result<bool> {
    let current_branch = get_current_branch(repo)?;
    Ok(current_branch == branch_name)
}

fn execute_command_at_path(
    cmd: &str,
    args: &[&str],
    repo_path: &Path,
) -> Result<std::process::Output> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .current_dir(repo_path.as_std_path())
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
        .default(0)
        .interact()?;

    match selection {
        0 => Ok(branch_name.to_string()),
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
    fn test_get_default_branch() {
        let temp_dir = TempDir::new().unwrap();

        // Setup test directory
        setup_test_git_repo(temp_dir.path()).unwrap();

        // Add a remote origin and set up origin/HEAD reference
        let output = Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed to add remote origin");

        // Create the refs/remotes/origin/HEAD symbolic reference pointing to main
        let refs_dir = temp_dir.path().join(".git/refs/remotes/origin");
        std::fs::create_dir_all(&refs_dir).unwrap();
        std::fs::write(refs_dir.join("HEAD"), "ref: refs/remotes/origin/main\n").unwrap();

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = get_default_branch(&repo);

        assert!(
            result.is_ok(),
            "Failed to get default branch: {:?}",
            result.err()
        );
        let default_branch = result.unwrap();
        assert_eq!(default_branch, Some("main".to_string()));
    }

    #[test]
    fn test_get_default_branch_with_develop() {
        let temp_dir = TempDir::new().unwrap();

        // Setup test directory
        setup_test_git_repo(temp_dir.path()).unwrap();

        // Add a remote origin and set up origin/HEAD reference pointing to develop
        let output = Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed to add remote origin");

        // Create the refs/remotes/origin/HEAD symbolic reference pointing to develop
        let refs_dir = temp_dir.path().join(".git/refs/remotes/origin");
        std::fs::create_dir_all(&refs_dir).unwrap();
        std::fs::write(refs_dir.join("HEAD"), "ref: refs/remotes/origin/develop\n").unwrap();

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = get_default_branch(&repo);

        assert!(
            result.is_ok(),
            "Failed to get default branch: {:?}",
            result.err()
        );
        let default_branch = result.unwrap();
        assert_eq!(default_branch, Some("develop".to_string()));
    }

    #[test]
    fn test_get_default_branch_no_remote_head() {
        let temp_dir = TempDir::new().unwrap();

        // Setup test directory without remote HEAD reference
        setup_test_git_repo(temp_dir.path()).unwrap();

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = get_default_branch(&repo);

        assert!(
            result.is_ok(),
            "Failed to get default branch: {:?}",
            result.err()
        );
        let default_branch = result.unwrap();
        // Should return None when no remote HEAD reference exists
        assert_eq!(default_branch, None);
    }

    #[test]
    fn test_get_default_branch_current_repo() {
        // Test with the actual current repository (git-qsync)
        let repo = gix::open(".").unwrap();
        let result = get_default_branch(&repo);

        assert!(
            result.is_ok(),
            "Failed to get default branch: {:?}",
            result.err()
        );
        let default_branch = result.unwrap();
        // Should detect "main" as the default branch for git-qsync repo
        assert_eq!(default_branch, Some("main".to_string()));
    }

    #[test]
    fn test_get_current_branch_on_main() {
        let temp_dir = TempDir::new().unwrap();

        // Setup test directory
        setup_test_git_repo(temp_dir.path()).unwrap();

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = get_current_branch(&repo);

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

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = get_current_branch(&repo);

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

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = get_current_branch(&repo);

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

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = check_branch_exists(&repo, "existing-branch");

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_check_branch_exists_false() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = check_branch_exists(&repo, "non-existent-branch");

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_is_branch_checked_out() {
        let temp_dir = TempDir::new().unwrap();

        setup_test_git_repo(temp_dir.path()).unwrap();
        create_branch(temp_dir.path(), "test-branch").unwrap();

        // Check that main/master is currently checked out
        let repo = gix::open(temp_dir.path()).unwrap();
        let current = get_current_branch(&repo).unwrap();

        let result = is_branch_checked_out(&repo, &current);
        assert!(
            result.is_ok(),
            "Failed to check if branch is checked out: {:?}",
            result.err()
        );
        assert!(result.unwrap());

        // Check that test-branch is not checked out
        let repo = gix::open(temp_dir.path()).unwrap();
        let result = is_branch_checked_out(&repo, "test-branch");
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
            {
                let repo = gix::open(temp_dir.path()).unwrap();
                check_branch_exists(&repo, "delete-me").unwrap()
            },
            "Branch should exist before deletion"
        );

        // Verify we're on a different branch (not the one we're about to delete)
        let repo = gix::open(temp_dir.path()).unwrap();
        let current_branch = get_current_branch(&repo).unwrap();
        assert_ne!(
            current_branch, "delete-me",
            "Should not be on the branch we're deleting"
        );

        let result = delete_branch_safely(&repo, "delete-me");

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
        let repo = gix::open(temp_dir.path()).unwrap();
        let current_branch = get_current_branch(&repo).unwrap();
        assert_eq!(
            current_branch, "feature-to-delete",
            "Should be on the branch we're about to delete"
        );

        // Verify the branch exists before trying to delete it
        assert!(
            {
                let repo = gix::open(temp_dir.path()).unwrap();
                check_branch_exists(&repo, "feature-to-delete").unwrap()
            },
            "Branch should exist before deletion"
        );

        // This should switch away and delete the branch
        let repo = gix::open(temp_dir.path()).unwrap();
        let result = delete_branch_safely(&repo, "feature-to-delete");

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

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = switch_to_branch(&repo, "switch-to-me");

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

        let repo = gix::open(temp_dir.path()).unwrap();
        let result = switch_to_branch(&repo, "non-existent-branch");

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
