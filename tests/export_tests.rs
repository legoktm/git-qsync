use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn setup_git_repo_with_branches(dir: &Path) {
    // Initialize repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("Failed to init git repo");
        
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("Failed to set git user.name");
        
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .expect("Failed to set git user.email");
    
    // Create main branch with initial commit
    fs::write(dir.join("README.md"), "# Test Repo").unwrap();
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(dir)
        .output()
        .expect("Failed to add file");
        
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .expect("Failed to commit");
    
    // Create and switch to feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature-test"])
        .current_dir(dir)
        .output()
        .expect("Failed to create feature branch");
    
    // Add a commit to feature branch
    fs::write(dir.join("feature.txt"), "Feature content").unwrap();
    Command::new("git")
        .args(["add", "feature.txt"])
        .current_dir(dir)
        .output()
        .expect("Failed to add feature file");
        
    Command::new("git")
        .args(["commit", "-m", "Add feature"])
        .current_dir(dir)
        .output()
        .expect("Failed to commit feature");
}

#[test]
fn test_export_bundle_creation() {
    let temp_dir = TempDir::new().unwrap();
    setup_git_repo_with_branches(temp_dir.path());
    
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();
    
    // Set up git config for testing
    Command::new("git")
        .args(["config", "qsync.target-vm", "test-vm"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to set target-vm config");
    
    // Test bundle creation logic by running git commands directly
    // Get merge base
    let merge_base_output = Command::new("git")
        .args(["merge-base", "feature-test", "HEAD~1"]) // HEAD~1 simulates main
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to get merge base");
    
    assert!(merge_base_output.status.success());
    let merge_base_string = String::from_utf8(merge_base_output.stdout).unwrap();
    let merge_base = merge_base_string.trim();
    
    // Create bundle
    let bundle_name = "test_feature-test_2024-01-01.bundle";
    let bundle_range = format!("{}..feature-test", merge_base);
    
    let bundle_output = Command::new("git")
        .args(["bundle", "create", bundle_name, &bundle_range])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create bundle");
    
    assert!(bundle_output.status.success());
    assert!(temp_dir.path().join(bundle_name).exists());
    
    // Verify bundle
    let verify_output = Command::new("git")
        .args(["bundle", "verify", bundle_name])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to verify bundle");
    
    assert!(verify_output.status.success());
    
    env::set_current_dir(original_dir).unwrap();
}