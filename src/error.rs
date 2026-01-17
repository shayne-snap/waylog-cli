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

    #[error("Missing required argument <AGENT>")]
    MissingAgent,

    #[error("{0} is not installed or not in PATH")]
    AgentNotInstalled(String),

    #[error("Child process exited with code {0}")]
    ChildProcessFailed(i32),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl WaylogError {
    /// Get the exit code for this error
    pub fn exit_code(&self) -> i32 {
        match self {
            // Command line usage errors
            WaylogError::MissingAgent | WaylogError::ProviderNotFound(_) => exitcode::USAGE,
            // Data format errors
            WaylogError::Json(_) => exitcode::DATAERR,
            // Input file/resource errors
            WaylogError::ProjectNotFound | WaylogError::Io(_) => exitcode::NOINPUT,
            // Service unavailable
            WaylogError::AgentNotInstalled(_) => exitcode::UNAVAILABLE,
            // Internal software errors
            WaylogError::PathError(_) | WaylogError::Internal(_) => exitcode::SOFTWARE,
            // Child process exit code (propagate directly)
            WaylogError::ChildProcessFailed(code) => *code,
        }
    }

    /// Check if this error type already has user-friendly output displayed
    /// Some errors (like MissingAgent, ProviderNotFound, AgentNotInstalled) are
    /// already displayed via output.error() in command handlers, so we shouldn't
    /// display them again in main.rs
    pub fn is_already_displayed(&self) -> bool {
        matches!(
            self,
            WaylogError::MissingAgent
                | WaylogError::ProviderNotFound(_)
                | WaylogError::AgentNotInstalled(_)
        )
    }
}

pub type Result<T> = std::result::Result<T, WaylogError>;
