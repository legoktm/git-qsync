use anyhow::{bail, Context, Result};

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
