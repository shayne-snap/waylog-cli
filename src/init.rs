use crate::cli::Commands;
use crate::error::Result;
use crate::output::Output;
use std::path::{Path, PathBuf};
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

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

/// Resolve the project root directory based on the command being executed.
/// Returns (project_root, is_new_project)
pub fn resolve_project_root(command: &Commands, output: &mut Output) -> Result<(PathBuf, bool)> {
    let found_root = crate::utils::path::find_project_root();

    match command {
        Commands::Pull { .. } => match found_root {
            Some(root) => {
                output.found_tracking(&root)?;
                Ok((root, false))
            }
            None => {
                // Interactive prompt for initialization
                let current_dir = std::env::current_dir()?;
                let waylog_path = current_dir.join(WAYLOG_DIR);

                output.not_initialized()?;
                output.init_prompt(&waylog_path)?;

                if dialoguer::Confirm::new()
                    .default(true)
                    .show_default(true)
                    .interact()
                    .unwrap_or(false)
                {
                    Ok((current_dir, true))
                } else {
                    output.aborted()?;
                    std::process::exit(0);
                }
            }
        },
        Commands::Run { .. } => match found_root {
            Some(root) => Ok((root, false)),
            None => {
                // For 'run', if no project found, initialize in current dir
                let current = std::env::current_dir()?;
                Ok((current, true))
            }
        },
    }
}

/// Setup logging system.
/// - Default: No file logging, no console output (tracing is disabled for console)
/// - With --verbose: Creates log file with detailed format, enables console tracing with simple format
/// - With --quiet: Completely silent (no tracing output at all)
pub fn setup_logging(project_root: &Path, verbose: bool, quiet: bool) -> Result<()> {
    // Determine log level based on verbose flag
    // Use RUST_LOG environment variable if set, otherwise use default based on verbose
    let default_log_level = if verbose { "debug" } else { "warn" };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_log_level));

    let base_subscriber = tracing_subscriber::registry().with(env_filter);

    // Build subscriber with conditional layers
    if verbose {
        let log_dir = project_root.join(WAYLOG_DIR).join(subdirs::LOGS);

        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&log_dir)?;

        // Create file appender (daily rotation)
        let file_appender = tracing_appender::rolling::daily(log_dir, WAYLOG_LOG_FILE);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // Keep the guard alive for the lifetime of the program
        // This ensures logs are flushed properly on exit
        // For a CLI tool, leaking this is acceptable as it will be cleaned up on program exit
        std::mem::forget(guard);

        // File logging: detailed format with timestamp, level, module, etc.
        let subscriber_with_file = base_subscriber.with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false) // No ANSI colors in log files
                .with_target(true) // Include module/target
                .with_file(true) // Include file name
                .with_line_number(true) // Include line number
                .with_thread_ids(false) // Don't include thread IDs (too verbose)
                .with_thread_names(false),
        );

        // Console logging: only if not quiet
        // Use simple format for console (just the message)
        if !quiet {
            let subscriber = subscriber_with_file.with(
                fmt::layer()
                    .with_writer(std::io::stderr) // Use stderr for logs
                    .with_target(false) // Don't show module in console
                    .with_file(false)
                    .with_line_number(false)
                    .with_thread_ids(false)
                    .with_thread_names(false)
                    .without_time(), // No timestamp in console (too verbose)
            );
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set tracing subscriber");
        } else {
            tracing::subscriber::set_global_default(subscriber_with_file)
                .expect("Failed to set tracing subscriber");
        }
    } else {
        // Default: no file logging, no console output
        tracing::subscriber::set_global_default(base_subscriber)
            .expect("Failed to set tracing subscriber");
    }

    Ok(())
}
