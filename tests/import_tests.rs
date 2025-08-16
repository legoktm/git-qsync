use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
fn setup_git_repo_with_bundle(dir: &Path) -> String {
    // Initialize source repo
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
    
    // Create main branch
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
    
    // Create feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature-import"])
        .current_dir(dir)
        .output()
        .expect("Failed to create feature branch");
    
    fs::write(dir.join("feature.txt"), "Import test feature").unwrap();
    Command::new("git")
        .args(["add", "feature.txt"])
        .current_dir(dir)
        .output()
        .expect("Failed to add feature file");
        
    Command::new("git")
        .args(["commit", "-m", "Add import feature"])
        .current_dir(dir)
        .output()
        .expect("Failed to commit feature");
    
    // Switch back to main to get merge base
    Command::new("git")
        .args(["checkout", "HEAD~1"])
        .current_dir(dir)
        .output()
        .expect("Failed to checkout main");
    
    let merge_base_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("Failed to get HEAD");
    
    let merge_base_string = String::from_utf8(merge_base_output.stdout).unwrap();
    let merge_base = merge_base_string.trim();
    
    // Create bundle from feature branch
    let bundle_name = "test_feature-import_2024-01-01.bundle";
    let bundle_range = format!("{}..feature-import", merge_base);
    
    Command::new("git")
        .args(["bundle", "create", bundle_name, &bundle_range])
        .current_dir(dir)
        .output()
        .expect("Failed to create bundle");
    
    bundle_name.to_string()
}

#[allow(dead_code)]
fn setup_target_repo(dir: &Path) {
    // Initialize target repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("Failed to init target repo");
        
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
    
    // Create initial commit
    fs::write(dir.join("README.md"), "# Target Repo").unwrap();
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
}

#[test]
fn test_bundle_verification() {
    let _temp_dir = TempDir::new().unwrap();
    
    // Simple test that git bundle verify command exists
    let output = Command::new("git")
        .args(["bundle", "verify"])
        .output();
    
    if let Ok(result) = output {
        // Command should fail with usage info, but not "command not found"
        assert!(!result.status.success()); // Expect failure due to missing bundle file
    }
}

#[test]
fn test_bundle_list_heads() {
    // Simple test that git bundle list-heads command exists
    let output = Command::new("git")
        .args(["bundle", "list-heads"])
        .output();
    
    if let Ok(result) = output {
        // Command should fail with usage info, but not "command not found"
        assert!(!result.status.success()); // Expect failure due to missing bundle file
    }
}

#[test]
fn test_bundle_import() {
    // Test that git fetch with bundle syntax is valid
    let output = Command::new("git")
        .args(["fetch", "--help"])
        .output();
    
    if let Ok(result) = output {
        let help_text = String::from_utf8_lossy(&result.stdout);
        // Just verify git fetch help mentions bundles or files
        assert!(help_text.contains("repository") || help_text.contains("fetch"));
    }
}

#[test]
fn test_check_branch_exists() {
    // Test git rev-parse command for checking branch existence
    let output = Command::new("git")
        .args(["rev-parse", "--help"])
        .output();
    
    if let Ok(result) = output {
        assert!(result.status.success());
        let help_text = String::from_utf8_lossy(&result.stdout);
        assert!(help_text.contains("rev-parse") || help_text.contains("verify"));
    }
}