use crate::error::{Result, WaylogError};
use crate::output::Output;
use crate::{exporter, providers, session, utils, watcher};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;

pub async fn handle_run(
    agent: Option<String>,
    args: Vec<String>,
    project_path: PathBuf,
    output: &mut Output,
) -> Result<()> {
    let agent_name = match agent {
        Some(name) => name,
        None => {
            output.missing_agent()?;
            std::process::exit(1);
        }
    };

    // Get and validate provider before calling run_agent
    let provider = match providers::get_provider(&agent_name) {
        Ok(p) => p,
        Err(WaylogError::ProviderNotFound(name)) => {
            output.unknown_agent(&name)?;
            std::process::exit(1);
        }
        Err(e) => return Err(e),
    };

    // Check if the tool is installed
    if !provider.is_installed() {
        output.agent_not_installed(provider.command())?;
        std::process::exit(1);
    }

    // Now run_agent can focus on execution without validation
    run_agent(args, project_path, provider).await?;

    Ok(())
}

async fn run_agent(
    args: Vec<String>,
    project_path: PathBuf,
    provider: Arc<dyn providers::base::Provider>,
) -> Result<()> {
    // Provider is already validated in handle_run, so we can focus on execution
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
