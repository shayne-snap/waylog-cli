use crate::error::Result;
use crate::{exporter, providers, session};
use std::sync::Arc;
use tokio::process::Child;
use tokio::task::JoinHandle;
use tracing;

/// Perform cleanup and final sync
///
/// This function handles:
/// - Stopping the file watcher
/// - Performing final sync of chat messages
/// - Saving session state
///
/// Errors during cleanup are logged but don't prevent the function from completing.
pub(crate) async fn cleanup_and_sync(
    watcher_handle: &JoinHandle<()>,
    _child: &mut Child,
    tracker: &Arc<session::SessionTracker>,
    provider: &Arc<dyn providers::base::Provider>,
    project_path: &std::path::Path,
    waylog_dir: &std::path::Path,
    _exit_status: Option<std::process::ExitStatus>,
) -> Result<()> {
    // Stop the file watcher
    watcher_handle.abort();
    // Wait a bit for the watcher to stop (non-blocking, ignore result)
    // Note: JoinHandle is not Copy, so we can't await the reference directly
    // Just abort is sufficient, the task will be cleaned up

    // Do a final sync
    tracing::info!("Session ended, performing final sync...");

    if let Ok(Some(session_file)) = provider.find_latest_session(project_path).await {
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

                // Perform sync - errors are logged but don't stop cleanup
                match (synced_count == 0, &markdown_path) {
                    (true, path) => {
                        if let Err(e) = exporter::create_markdown_file(path, &session).await {
                            tracing::error!("Failed to create markdown file: {}", e);
                        }
                    }
                    (false, path) => {
                        if let Err(e) = exporter::append_messages(path, &new_messages).await {
                            tracing::error!("Failed to append messages: {}", e);
                        }
                    }
                }

                if let Err(e) = tracker
                    .update_session(
                        session.session_id.clone(),
                        session_file,
                        markdown_path.clone(),
                        session.messages.len(),
                    )
                    .await
                {
                    tracing::error!("Failed to update session: {}", e);
                } else {
                    tracing::info!("✓ Final sync complete: {}", markdown_path.display());
                }
            }
        }
    }

    // Save final state - errors are logged but don't stop cleanup
    if let Err(e) = tracker.save_state().await {
        tracing::warn!("Failed to save state: {}", e);
    }

    Ok(())
}
