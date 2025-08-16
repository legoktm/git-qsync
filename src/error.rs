use thiserror::Error;

#[derive(Error, Debug)]
pub enum QSyncError {
    #[error("Not in a git repository")]
    NotInGitRepo,
    
    #[error("No bundle files found in {path}")]
    NoBundlesFound { path: String },
    
    #[error("Bundle verification failed")]
    BundleVerificationFailed,
    
    #[error("Git command failed: {message}")]
    GitCommandFailed { message: String },
    
    #[error("Configuration missing: {key}")]
    ConfigMissing { key: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Cancelled by user")]
    Cancelled,
}