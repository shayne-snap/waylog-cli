mod cli;
mod error;
mod exporter;
mod providers;
mod session;
pub mod synchronizer;
mod utils;
mod watcher;

use clap::Parser;
use cli::{Cli, Commands};
use error::Result;
use std::sync::Arc;
use tracing::debug;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Detect project root (find .waylog or .git)
    let project_root = utils::path::find_project_root();
    let log_dir = project_root.join(".waylog").join("logs");

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Create file appender (daily rotation)
    let file_appender = tracing_appender::rolling::daily(log_dir, "waylog.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let cli = Cli::parse();

    // Determine log level based on verbose flag
    // Currently EnvFilter::from_default_env takes precedence if RUST_LOG is set
    // Otherwise fallback to "debug" if verbose is true, or "info" if false
    let log_level = if cli.verbose { "debug" } else { "info" };

    // Initialize logging to both console and file
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)))
        .with(fmt::layer().with_writer(std::io::stdout)) // Console
        .with(fmt::layer().with_writer(non_blocking)); // File

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    match cli.command {
        Commands::Run { agent, args } => {
            if let Some(agent_name) = agent {
                if let Err(e) = run_agent(&agent_name, args).await {
                    match e {
                        error::WaylogError::ProviderNotFound(name) => {
                            eprintln!("Error: '{}' is not a recognized agent.\n", name);
                            eprintln!("Available agents:");
                            for provider in providers::list_providers() {
                                eprintln!("- {}", provider);
                            }
                            eprintln!("\nDid you mean to run 'waylog pull'?");
                            std::process::exit(1);
                        }
                        _ => return Err(e),
                    }
                }
            } else {
                eprintln!("Error: Missing required argument <AGENT>\n");
                eprintln!("Usage: waylog run <AGENT> [ARGS]...\n");
                eprintln!("Available agents:");
                for provider in providers::list_providers() {
                    eprintln!("- {}", provider);
                }
                eprintln!("\nExample:\n  waylog run claude");
                std::process::exit(1);
            }
        }
        Commands::Pull { provider, force } => {
            pull_history(provider, force, cli.verbose).await?;
        }
    }

    Ok(())
}

async fn pull_history(provider_name: Option<String>, force: bool, verbose: bool) -> Result<()> {
    use synchronizer::SyncStatus;

    // Detect project root
    let project_path = utils::path::find_project_root();
    println!(
        "Pulling chat history for project: {}",
        project_path.display()
    );

    // Filter providers
    let providers_to_sync = if let Some(name) = provider_name {
        vec![providers::get_provider(&name)?]
    } else {
        // Sync all known providers
        vec![
            providers::get_provider("claude")?,
            providers::get_provider("gemini")?,
            providers::get_provider("codex")?,
        ]
    };

    let mut total_synced = 0;
    let mut total_uptodate = 0;

    for provider in providers_to_sync {
        if !provider.is_installed() {
            debug!("Skipping {} (not installed)", provider.name());
            continue;
        }

        // Create session tracker and synchronizer
        let tracker =
            Arc::new(session::SessionTracker::new(project_path.clone(), provider.clone()).await?);
        let synchronizer = synchronizer::Synchronizer::new(
            provider.clone(),
            project_path.clone(),
            tracker.clone(),
        );

        match synchronizer.sync_all(force).await {
            Ok(results) => {
                // Print section header
                println!("\n[{}] Found {} sessions", provider.name(), results.len());

                let mut provider_uptodate = 0;
                let mut provider_synced = 0;
                let mut provider_skipped = 0;
                let mut _provider_failed = 0;

                for (path, status) in results {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    match status {
                        SyncStatus::Synced { new_messages } => {
                            if verbose {
                                println!(
                                    "  ↑ Synced: {} ({} new messages)",
                                    filename, new_messages
                                );
                            }
                            provider_synced += 1;
                        }
                        SyncStatus::UpToDate => {
                            if verbose {
                                println!("  ✓ Up to date: {}", filename);
                            }
                            provider_uptodate += 1;
                        }
                        SyncStatus::Failed(e) => {
                            println!("  ✗ Failed to sync {}: {}", filename, e);
                            _provider_failed += 1;
                        }
                        SyncStatus::Skipped => {
                            if verbose {
                                println!("  ⊘ Skipped: {} (empty or invalid session)", filename);
                            }
                            provider_skipped += 1;
                        }
                    }
                }

                if !verbose {
                    if provider_synced > 0 {
                        println!("  ↑ {} sessions synced", provider_synced);
                    }
                    if provider_uptodate > 0 {
                        println!("  ✓ {} sessions up to date", provider_uptodate);
                    }
                }
                if verbose && provider_skipped > 0 {
                    println!("  ⊘ {} sessions skipped", provider_skipped);
                }

                total_synced += provider_synced;
                total_uptodate += provider_uptodate;
            }
            Err(e) => {
                tracing::error!("Failed to scan {}: {}", provider.name(), e);
            }
        }

        // Save state after each provider
        tracker.save_state().await?;
    }

    println!(
        "\n✨ Pull complete! {} sessions updated, {} up to date.",
        total_synced, total_uptodate
    );

    Ok(())
}
async fn run_agent(agent: &str, args: Vec<String>) -> Result<()> {
    use std::process::Stdio;
    use tokio::process::Command;

    // Get the provider
    let provider = providers::get_provider(agent)?;

    // Check if the tool is installed
    if !provider.is_installed() {
        eprintln!(
            "Error: {} is not installed or not in PATH",
            provider.command()
        );
        eprintln!("Please install it first before using waylog.");
        std::process::exit(1);
    }

    // Detect project root
    let project_path = utils::path::find_project_root();
    tracing::info!("Starting {} in {}", provider.name(), project_path.display());

    // Ensure .waylog/history directory exists
    let waylog_dir = utils::path::get_waylog_dir(&project_path);
    utils::path::ensure_dir_exists(&waylog_dir)?;

    tracing::info!("Chat history will be saved to: {}", waylog_dir.display());

    // Create session tracker
    let tracker =
        Arc::new(session::SessionTracker::new(project_path.clone(), provider.clone()).await?);

    // Create file watcher
    let watcher =
        watcher::FileWatcher::new(provider.clone(), project_path.clone(), tracker.clone());

    // Start file watcher in background
    let watcher_handle = tokio::spawn(async move {
        if let Err(e) = watcher.watch().await {
            tracing::error!("File watcher error: {}", e);
        }
    });

    // Start the AI CLI tool as a child process
    tracing::info!("Launching {}...", provider.command());
    let mut child = Command::new(provider.command())
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    // Wait for the child process to exit
    let status = child.wait().await?;

    // Stop the file watcher
    watcher_handle.abort();

    // Do a final sync
    tracing::info!("Session ended, performing final sync...");
    if let Ok(Some(session_file)) = provider.find_latest_session(&project_path).await {
        if let Ok((session, new_messages)) = tracker.get_new_messages(&session_file).await {
            if !new_messages.is_empty() {
                tracing::info!("Syncing {} final messages", new_messages.len());

                let markdown_path =
                    if let Some(existing) = tracker.get_markdown_path(&session.session_id).await {
                        existing
                    } else {
                        let slug = session
                            .messages
                            .iter()
                            .find(|m| m.role == crate::providers::base::MessageRole::User)
                            .map(|m| crate::utils::string::slugify(&m.content))
                            .unwrap_or_else(|| session.session_id.clone());

                        let timestamp = session.started_at.format("%Y-%m-%d_%H-%M-%SZ");
                        let filename = format!("{}-{}-{}.md", timestamp, provider.name(), slug);
                        waylog_dir.join(filename)
                    };

                let synced_count = tracker.get_synced_count(&session.session_id).await;
                if synced_count == 0 {
                    exporter::create_markdown_file(&markdown_path, &session).await?;
                } else {
                    exporter::append_messages(&markdown_path, &new_messages).await?;
                }

                tracker
                    .update_session(
                        session.session_id.clone(),
                        session_file,
                        markdown_path.clone(),
                        session.messages.len(),
                    )
                    .await?;

                tracing::info!("✓ Final sync complete: {}", markdown_path.display());
            }
        }
    }

    // Save final state
    tracker.save_state().await?;

    if !status.success() {
        tracing::warn!("{} exited with status: {:?}", provider.name(), status);
    }

    tracing::info!(
        "Session complete. Chat history saved to: {}",
        waylog_dir.display()
    );
    Ok(())
}
