/// Configuration constants for waylog paths and directories
/// The name of the waylog project directory (e.g., `.waylog`)
pub const WAYLOG_DIR: &str = ".waylog";

/// The name of the waylog log file
pub const WAYLOG_LOG_FILE: &str = "waylog.log";

/// Subdirectories within .waylog
pub mod subdirs {
    /// History directory for markdown files
    pub const HISTORY: &str = "history";

    /// Logs directory for log files
    pub const LOGS: &str = "logs";
}
