use crate::command_utils::execute_command;
use crate::config::{check_git_repo, get_current_branch, get_project_name};
use anyhow::{bail, Context, Result};
use jiff::Zoned;

pub(crate) fn run(branch: Option<String>) -> Result<()> {
    check_git_repo()?;

    let branch_name = match branch {
        Some(b) => b,
        None => get_current_branch()?,
    };

    let project_name = get_project_name()?;
    let timestamp = Zoned::now().strftime("%Y-%m-%dT%H-%M-%S").to_string();
    // Sanitize branch name for filename (replace / with -)
    let safe_branch_name = branch_name.replace('/', "-");
    let bundle_filename = format!("{}_{}_{}.bundle", project_name, safe_branch_name, timestamp);

    println!("Exporting branch '{}'...", branch_name);

    // Get default branch and merge base
    let default_branch = get_default_branch()?;

    // Create bundle range - for default branch, export entire history
    let bundle_range = if is_default_branch(&branch_name, &default_branch)? {
        println!(
            "Exporting entire branch history for default branch '{}'",
            branch_name
        );
        branch_name.clone()
    } else {
        let merge_base = get_merge_base(&branch_name, &default_branch)?;
        format!("{}..{}", merge_base, branch_name)
    };
    create_bundle(&bundle_filename, &bundle_range)?;

    // Move bundle (qvm-move will prompt for target VM)
    move_bundle_to_vm(&bundle_filename)?;

    println!("Successfully exported branch '{}'", branch_name);

    Ok(())
}

fn is_default_branch(branch: &str, default_branch: &str) -> Result<bool> {
    // Normalize branch names for comparison
    let normalized_branch = branch.strip_prefix("refs/heads/").unwrap_or(branch);
    let normalized_default = default_branch
        .strip_prefix("origin/")
        .or_else(|| default_branch.strip_prefix("refs/remotes/origin/"))
        .or_else(|| default_branch.strip_prefix("refs/heads/"))
        .unwrap_or(default_branch);

    Ok(normalized_branch == normalized_default)
}

fn get_default_branch() -> Result<String> {
    let output = execute_command("git", &["symbolic-ref", "refs/remotes/origin/HEAD"])?;

    if !output.status.success() {
        let repo = gix::discover(".")?;

        // Fallback to main/master (remote first, then local)
        if repo.find_reference("refs/remotes/origin/main").is_ok() {
            return Ok("origin/main".to_string());
        }

        if repo.find_reference("refs/remotes/origin/master").is_ok() {
            return Ok("origin/master".to_string());
        }

        // Try local branches if no remote
        if repo.find_reference("refs/heads/main").is_ok() {
            return Ok("main".to_string());
        }

        if repo.find_reference("refs/heads/master").is_ok() {
            return Ok("master".to_string());
        }

        bail!("Cannot determine default branch");
    }

    let default_ref = String::from_utf8(output.stdout)?.trim().to_string();

    // Extract branch name from refs/remotes/origin/main
    let branch = default_ref
        .strip_prefix("refs/remotes/")
        .unwrap_or(&default_ref);

    Ok(branch.to_string())
}

fn get_merge_base(branch: &str, default_branch: &str) -> Result<String> {
    let repo = gix::discover(".")?;

    // Find the commit objects for both branches
    let branch_commit = repo
        .rev_parse_single(branch)?
        .object()?
        .try_into_commit()
        .map_err(|_| anyhow::anyhow!("Branch {} does not point to a commit", branch))?;

    let default_commit = repo
        .rev_parse_single(default_branch)?
        .object()?
        .try_into_commit()
        .map_err(|_| anyhow::anyhow!("Branch {} does not point to a commit", default_branch))?;

    // Find merge base - repo.merge_base returns a single gix::Id, not a Vec
    let merge_base = repo
        .merge_base(branch_commit.id(), default_commit.id())
        .with_context(|| {
            format!(
                "Cannot find merge base between {} and {}",
                branch, default_branch
            )
        })?;

    Ok(merge_base.to_string())
}

fn create_bundle(filename: &str, range: &str) -> Result<()> {
    let output = execute_command("git", &["bundle", "create", filename, range])?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Git command failed: Bundle creation failed: {}", error_msg);
    }

    Ok(())
}

fn move_bundle_to_vm(filename: &str) -> Result<()> {
    let output = execute_command("qvm-move", &[filename])?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Git command failed: qvm-move failed: {}", error_msg);
    }

    Ok(())
}
