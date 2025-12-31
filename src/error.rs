use thiserror::Error;

#[derive(Error, Debug)]
pub enum WaylogError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Path error: {0}")]
    PathError(String),

    #[error("No waylog project found.\nTo start a new session, use `waylog run <AGENT>`.\nTo sync history, please run this command inside an active waylog project (.waylog folder found).")]
    ProjectNotFound,
}

pub type Result<T> = std::result::Result<T, WaylogError>;
