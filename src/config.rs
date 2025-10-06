use anyhow::{bail, Context, Result};
use camino::Utf8Path as Path;

pub(crate) struct Config {
    pub(crate) source_vm: Option<String>,
}

impl Config {
    pub(crate) fn load() -> Result<Self> {
        let source_vm = get_git_config("qsync.source-vm")?;

        Ok(Config { source_vm })
    }

    pub(crate) fn get_source_vm(&self) -> Result<String> {
        self.source_vm
            .clone()
            .context("Configuration missing: qsync.source-vm")
    }
}

fn get_git_config(key: &str) -> Result<Option<String>> {
    let repo = gix::discover(".")?;
    let config = repo.config_snapshot();

    match config.string(key) {
        Some(value) => {
            let value_str = value.to_string();
            Ok(if value_str.is_empty() {
                None
            } else {
                Some(value_str)
            })
        }
        None => Ok(None), // Key not found
    }
}

pub(crate) fn get_project_name() -> Result<String> {
    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("Cannot determine project name from current directory")?;

    Ok(project_name.to_string())
}

pub(crate) fn get_current_branch() -> Result<String> {
    let repo = gix::discover(".")?;
    get_current_branch_from_repo(&repo)
}

pub(crate) fn get_current_branch_from_repo(repo: &gix::Repository) -> Result<String> {
    let head_ref = repo.head_ref()?;

    match head_ref {
        Some(reference) => {
            let name = reference.name().shorten().to_string();
            Ok(name)
        }
        None => bail!("HEAD is detached or repository is in an invalid state"),
    }
}

pub(crate) fn check_git_repo() -> Result<()> {
    gix::discover(".")
        .map(|_| ())
        .context("Not in a git repository")
}

pub(crate) fn get_default_branch() -> Result<String> {
    let output = crate::command_utils::execute_command(
        "git",
        &["symbolic-ref", "refs/remotes/origin/HEAD"],
        Path::new("."),
    )?;

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

pub(crate) fn get_default_branch_from_repo(repo: &gix::Repository) -> Result<Option<String>> {
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
