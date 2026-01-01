use crate::config::{subdirs, WAYLOG_DIR, WAYLOG_LOG_FILE};
use crate::error::Result;
use std::path::Path;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

/// Setup logging system with both console and file output.
/// Creates the log directory if it doesn't exist.
pub fn setup_logging(project_root: &Path, verbose: bool) -> Result<()> {
    let log_dir = project_root.join(WAYLOG_DIR).join(subdirs::LOGS);

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Create file appender (daily rotation)
    let file_appender = tracing_appender::rolling::daily(log_dir, WAYLOG_LOG_FILE);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Determine log level based on verbose flag
    let log_level = if verbose { "debug" } else { "info" };

    // Initialize logging to both console and file
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)))
        .with(fmt::layer().with_writer(std::io::stdout)) // Console
        .with(fmt::layer().with_writer(non_blocking)); // File

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    Ok(())
}
