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

    // Get default branch and merge base
    let default_branch = get_default_branch()?;

    // Create bundle range - for default branch, export entire history
    let bundle_range = if is_default_branch(&branch_name, &default_branch)? {
        let (commit_count, first_sha, last_sha) = get_branch_info(&branch_name)?;
        println!(
            "Exporting branch '{}' ({} commits: {}..{})",
            branch_name, commit_count, first_sha, last_sha
        );
        branch_name.clone()
    } else {
        let merge_base = get_merge_base(&branch_name, &default_branch)?;
        let range = format!("{}..{}", merge_base, branch_name);
        let (commit_count, first_sha, last_sha) = get_range_info(&range)?;
        println!(
            "Exporting branch '{}' ({} commits: {}..{})",
            branch_name, commit_count, first_sha, last_sha
        );
        range
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

fn get_branch_info(branch_name: &str) -> Result<(usize, String, String)> {
    let repo = gix::discover(".")?;

    // Get the branch commit
    let branch_commit = repo
        .rev_parse_single(branch_name)?
        .object()?
        .try_into_commit()
        .map_err(|_| anyhow::anyhow!("Branch {} does not point to a commit", branch_name))?;

    let last_sha = branch_commit.id().to_string()[..8].to_string();

    // Walk the commit history to count and get first/last
    let mut count = 0;
    let mut first_sha = String::new();
    let revwalk = repo.rev_walk([branch_commit.id()]);

    for commit_info in revwalk.all()? {
        let commit_info = commit_info?;
        count += 1;
        first_sha = commit_info.id.to_string()[..8].to_string(); // This will be the last one (oldest)

        // Limit to avoid issues on very large repos
        if count > 50000 {
            break;
        }
    }

    if count == 0 {
        return Ok((0, "no commits".to_string(), "no commits".to_string()));
    }

    Ok((count, first_sha, last_sha))
}

fn get_range_info(range: &str) -> Result<(usize, String, String)> {
    let repo = gix::discover(".")?;

    // Parse the range (e.g., "abc123..def456")
    if !range.contains("..") {
        bail!("Invalid range format: {}", range);
    }

    let parts: Vec<&str> = range.split("..").collect();
    if parts.len() != 2 {
        bail!("Invalid range format: {}", range);
    }

    let start_commit = repo
        .rev_parse_single(parts[0])?
        .object()?
        .try_into_commit()
        .map_err(|_| anyhow::anyhow!("Start of range {} does not point to a commit", parts[0]))?;

    let end_commit = repo
        .rev_parse_single(parts[1])?
        .object()?
        .try_into_commit()
        .map_err(|_| anyhow::anyhow!("End of range {} does not point to a commit", parts[1]))?;

    // Walk from end_commit, excluding start_commit's ancestors
    let mut count = 0;
    let mut first_sha = String::new();
    let last_sha = end_commit.id().to_string()[..8].to_string();

    // Use gix's revision walking to get commits in the range
    let revwalk = repo
        .rev_walk([end_commit.id()])
        .with_hidden([start_commit.id()]);

    for commit_info in revwalk.all()? {
        let commit_info = commit_info?;
        count += 1;
        first_sha = commit_info.id.to_string()[..8].to_string(); // This will be the last one (oldest in range)

        // Limit to avoid issues on very large repos
        if count > 50000 {
            break;
        }
    }

    if count == 0 {
        return Ok((0, "no commits".to_string(), "no commits".to_string()));
    }

    Ok((count, first_sha, last_sha))
}
