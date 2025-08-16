use crate::command_utils::execute_command;
use crate::config::{check_git_repo, get_project_name, Config};
use anyhow::{bail, Context, Result};
use dialoguer::{Confirm, Select};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub(crate) fn run(bundle_file: Option<String>) -> Result<()> {
    check_git_repo()?;

    let config = Config::load()?;
    let source_vm = config.get_source_vm()?;
    let project_name = get_project_name()?;

    let bundle_path = match bundle_file {
        Some(file) => PathBuf::from(file),
        None => {
            let qubes_incoming_path = format!(
                "{}/QubesIncoming/{}/{}",
                std::env::var("HOME")?,
                source_vm,
                project_name
            );
            find_latest_bundle(&qubes_incoming_path)?
        }
    };

    println!("Found bundle: {}", bundle_path.display());

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

    // Import the bundle
    import_bundle(&bundle_path, &branch_name, &final_branch_name)?;

    println!(
        "Successfully imported branch '{}' as '{}'",
        branch_name, final_branch_name
    );

    Ok(())
}

fn find_latest_bundle(dir_path: &str) -> Result<PathBuf> {
    let path = Path::new(dir_path);

    if !path.exists() {
        bail!("No bundle files found in {}", dir_path);
    }

    let mut bundles = Vec::new();

    for entry in WalkDir::new(path).max_depth(1) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "bundle" {
                    bundles.push(entry.into_path());
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
    let output = execute_command("git", &["bundle", "verify", bundle_path.to_str().unwrap()])?;

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
    let output = execute_command(
        "git",
        &["bundle", "list-heads", bundle_path.to_str().unwrap()],
    )?;

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
    let output = execute_command(
        "git",
        &[
            "rev-parse",
            "--verify",
            &format!("refs/heads/{}", branch_name),
        ],
    )?;

    Ok(output.status.success())
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
    let bundle_str = bundle_path.to_str().unwrap();
    let refspec = format!(
        "refs/heads/{}:refs/heads/{}",
        original_branch, target_branch
    );

    let output = execute_command("git", &["fetch", bundle_str, &refspec])?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Git command failed: Failed to import bundle: {}", error_msg);
    }

    Ok(())
}
