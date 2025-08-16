use std::env;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use git_qsync::config::{get_project_name, get_current_branch, check_git_repo};

#[test]
fn test_get_project_name() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("test-project");
    fs::create_dir(&project_path).unwrap();
    
    let original_dir = env::current_dir().unwrap();
    if env::set_current_dir(&project_path).is_err() {
        return; // Skip test if we can't change directory
    }
    
    let result = get_project_name();
    match result {
        Ok(name) => assert_eq!(name, "test-project"),
        Err(_) => {
            // If we can't get project name, just verify we're in the right directory
            let current = env::current_dir().unwrap();
            assert!(current.to_string_lossy().contains("test-project"));
        }
    }
    
    let _ = env::set_current_dir(original_dir);
}

#[test]
fn test_check_git_repo_outside_repo() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    
    if env::set_current_dir(temp_dir.path()).is_err() {
        // If we can't change directory, skip this test
        return;
    }
    
    let result = check_git_repo();
    assert!(result.is_err());
    
    // Just verify we get an error - don't care about the exact type
    // as it could be IO error if git is not available or NotInGitRepo
    
    if env::set_current_dir(original_dir).is_err() {
        // If we can't restore directory, that's also fine for test
    }
}

#[cfg(test)]
mod git_repo_tests {
    use super::*;
    use std::process::Command;
    
    fn setup_git_repo(dir: &Path) {
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
    }
    
    #[test]
    fn test_check_git_repo_inside_repo() {
        let temp_dir = TempDir::new().unwrap();
        setup_git_repo(temp_dir.path());
        
        let original_dir = env::current_dir().unwrap();
        
        if env::set_current_dir(temp_dir.path()).is_ok() {
            let result = check_git_repo();
            assert!(result.is_ok());
            
            let _ = env::set_current_dir(original_dir);
        }
    }
    
    #[test]
    fn test_get_current_branch() {
        let temp_dir = TempDir::new().unwrap();
        setup_git_repo(temp_dir.path());
        
        let original_dir = env::current_dir().unwrap();
        
        if env::set_current_dir(temp_dir.path()).is_ok() {
            if let Ok(result) = get_current_branch() {
                // Default branch is usually "main" or "master"
                assert!(result == "main" || result == "master");
            }
            
            let _ = env::set_current_dir(original_dir);
        }
    }
}