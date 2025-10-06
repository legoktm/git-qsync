use std::process::Command as StdCommand;

pub fn setup_test_git_repo(dir: &std::path::Path) {
    StdCommand::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("Failed to init git repo");

    StdCommand::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("Failed to set git user.name");

    StdCommand::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .expect("Failed to set git user.email");

    std::fs::write(dir.join("README.md"), "# Test Repo").unwrap();
    StdCommand::new("git")
        .args(["add", "README.md"])
        .current_dir(dir)
        .output()
        .expect("Failed to add file");

    StdCommand::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .expect("Failed to commit");
}
