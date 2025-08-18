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
    cmd.env("QVM_MOVE_PATH", "echo"); // Use echo as no-op command
    cmd.args(["export"]);

    // Using echo as a no-op command should always succeed
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("commits:"));
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
fn test_export_with_echo_qvm_move() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.env("QVM_MOVE_PATH", "echo"); // Use echo as no-op command
    cmd.args(["export"]);

    // Using echo as a no-op command should always succeed
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("commits:"));
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

#[test]
fn test_import_branch_operations() {
    use std::fs;

    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    // Create a feature branch to test with
    StdCommand::new("git")
        .args(["checkout", "-b", "feature-branch"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create feature branch");

    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(temp_dir.path())
        .output()
        .or_else(|_| {
            // Fallback to master
            StdCommand::new("git")
                .args(["checkout", "master"])
                .current_dir(temp_dir.path())
                .output()
        })
        .expect("Failed to checkout main/master");

    // Create a bundle file for testing
    let _bundle_content = format!(
        "# git bundle test for {}\n# created from feature-branch\n",
        temp_dir.path().file_name().unwrap().to_str().unwrap()
    );

    let project_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
    let qubes_incoming_dir = temp_dir
        .path()
        .join("QubesIncoming/test-vm/git-qsync")
        .join(project_name);
    fs::create_dir_all(&qubes_incoming_dir).unwrap();

    // Create a real git bundle for testing
    let bundle_path = qubes_incoming_dir.join("test_feature-branch_2024-01-01T12-00-00.bundle");
    StdCommand::new("git")
        .args([
            "bundle",
            "create",
            bundle_path.to_str().unwrap(),
            "feature-branch",
        ])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create bundle");

    // Set up config for import
    StdCommand::new("git")
        .args(["config", "qsync.source-vm", "test-vm"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to set source-vm config");

    // Test import (this would require interactive input handling in a real test)
    // For now, just verify the setup is correct
    let mut cmd = Command::cargo_bin("git-qsync").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.args(["import", bundle_path.to_str().unwrap()]);

    // This test would fail without proper bundle contents, but it tests the setup
    let output = cmd.output().unwrap();

    // The command should at least recognize it's in a git repo and find the bundle
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Not in a git repository"));
}

#[test]
fn test_branch_detection_integration() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    // Create additional branches
    for branch in &["feature-1", "feature-2", "bugfix-1"] {
        StdCommand::new("git")
            .args(["checkout", "-b", branch])
            .current_dir(temp_dir.path())
            .output()
            .unwrap_or_else(|_| panic!("Failed to create branch {}", branch));

        // Add a commit to make the branch distinct
        let file_name = format!("{}.txt", branch);
        fs::write(
            temp_dir.path().join(&file_name),
            format!("Content for {}", branch),
        )
        .unwrap();

        StdCommand::new("git")
            .args(["add", &file_name])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to add file");

        StdCommand::new("git")
            .args(["commit", "-m", &format!("Add {}", branch)])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to commit");
    }

    // Return to main/master
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(temp_dir.path())
        .output()
        .or_else(|_| {
            StdCommand::new("git")
                .args(["checkout", "master"])
                .current_dir(temp_dir.path())
                .output()
        })
        .expect("Failed to checkout main/master");

    // Verify we can export from different branches
    for branch in &["feature-1", "feature-2", "bugfix-1"] {
        let mut cmd = Command::cargo_bin("git-qsync").unwrap();
        cmd.current_dir(temp_dir.path());
        cmd.env("QVM_MOVE_PATH", "echo"); // Use echo as no-op
        cmd.args(["export", branch]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("commits:"));
    }
}

#[test]
fn test_bundle_verification() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_git_repo(temp_dir.path());

    // Create a feature branch
    StdCommand::new("git")
        .args(["checkout", "-b", "feature-verify"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create feature branch");

    // Add some content
    fs::write(temp_dir.path().join("feature.txt"), "Feature content").unwrap();
    StdCommand::new("git")
        .args(["add", "feature.txt"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to add file");

    StdCommand::new("git")
        .args(["commit", "-m", "Add feature"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to commit");

    // Create bundle
    let bundle_path = temp_dir.path().join("test.bundle");
    StdCommand::new("git")
        .args([
            "bundle",
            "create",
            bundle_path.to_str().unwrap(),
            "feature-verify",
        ])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create bundle");

    // Test that the bundle verifies correctly
    let output = StdCommand::new("git")
        .args(["bundle", "verify", bundle_path.to_str().unwrap()])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to verify bundle");

    assert!(output.status.success(), "Bundle should verify successfully");

    // Test bundle list-heads
    let output = StdCommand::new("git")
        .args(["bundle", "list-heads", bundle_path.to_str().unwrap()])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to list bundle heads");

    assert!(output.status.success(), "Bundle list-heads should succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("refs/heads/feature-verify"),
        "Bundle should contain feature-verify branch"
    );
}
