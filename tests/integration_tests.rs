use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn setup_test_git_repo(dir: &std::path::Path) {
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

    fs::write(dir.join("README.md"), "# Test Repo").unwrap();
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

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains(
        "Transfer git branches between Qubes VMs",
    ));
}

#[test]
fn test_export_help() {
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.args(["export", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Branch to export"));
}

#[test]
fn test_import_help() {
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.args(["import", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Specific bundle file to import"));
}

#[test]
fn test_export_outside_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.args(["export"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));
}

#[test]
fn test_import_outside_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.args(["import"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));
}

#[test]
fn test_export_without_qvm_move() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.args(["export"]);
    cmd.assert().failure().stderr(
        predicate::str::contains("qvm-move").or(predicate::str::contains("Bundle creation failed")),
    );
}

#[test]
fn test_import_missing_config() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.args(["import"]);
    cmd.assert().failure().stderr(predicate::str::contains(
        "Configuration missing: qsync.source-vm",
    ));
}

#[test]
fn test_export_with_no_qvm_move() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.args(["export"]);
    cmd.assert().failure().stderr(
        predicate::str::contains("qvm-move").or(predicate::str::contains("Bundle creation failed")),
    );
}

#[test]
fn test_alias_commands() {
    // Test export alias
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.args(["e", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Branch to export"));

    // Test import alias
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.args(["i", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Specific bundle file to import"));
}

#[test]
fn test_init_command() {
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.args(["init"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "Git aliases configured successfully!",
    ));
}

#[test]
fn test_invalid_subcommand() {
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.args(["invalid"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}
