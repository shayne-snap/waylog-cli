mod cleanup;
mod process;

use crate::error::{Result, WaylogError};
use crate::output::Output;
use crate::{providers, session, utils, watcher};
use futures::stream::StreamExt;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook_tokio::Signals;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::task::JoinHandle;

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
            return Err(WaylogError::MissingAgent);
        }
    };

    // Get and validate provider before calling run_agent
    let provider = match providers::get_provider(&agent_name) {
        Ok(p) => p,
        Err(WaylogError::ProviderNotFound(name)) => {
            output.unknown_agent(&name)?;
            return Err(WaylogError::ProviderNotFound(name));
        }
        Err(e) => return Err(e),
    };

    // Check if the tool is installed
    if !provider.is_installed() {
        output.agent_not_installed(provider.command())?;
        return Err(WaylogError::AgentNotInstalled(
            provider.command().to_string(),
        ));
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
    let watcher_handle: JoinHandle<()> = tokio::spawn(async move {
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

    // Setup signal handling
    let mut signals = match Signals::new([SIGINT, SIGTERM]) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                "Failed to setup signal handling: {}. Continuing without signal support.",
                e
            );
            // Fallback to original behavior without signal handling
            let status = child.wait().await?;
            watcher_handle.abort();
            cleanup::cleanup_and_sync(
                &watcher_handle,
                &mut child,
                &tracker,
                &provider,
                &project_path,
                &waylog_dir,
                Some(status),
            )
            .await?;
            // Propagate child process exit code
            if !status.success() {
                let exit_code = status.code().unwrap_or(1);
                return Err(WaylogError::ChildProcessFailed(exit_code));
            }
            return Ok(());
        }
    };

    // Use select! to wait for either signal or child process exit
    let exit_status = tokio::select! {
        // Signal received
        signal_result = signals.next() => {
            if let Some(sig) = signal_result {
                let signal_name = match sig {
                    SIGINT => "SIGINT (Ctrl+C)",
                    SIGTERM => "SIGTERM",
                    _ => "unknown signal",
                };
                tracing::info!("Received {}, cleaning up...", signal_name);

                // Terminate child process
                process::terminate_child(&mut child).await;

                // Wait for child to exit and get its status
                let status = child.wait().await?;

                // Perform cleanup
                cleanup::cleanup_and_sync(
                    &watcher_handle,
                    &mut child,
                    &tracker,
                    &provider,
                    &project_path,
                    &waylog_dir,
                    Some(status),
                )
                .await?;

                // Propagate child process exit code (or signal exit code)
                // Standard exit codes: 130 for SIGINT, 143 for SIGTERM
                let exit_code = match sig {
                    SIGINT => 130,
                    SIGTERM => 143,
                    _ => status.code().unwrap_or(1),
                };
                return Err(WaylogError::ChildProcessFailed(exit_code));
            } else {
                // Signals stream ended unexpectedly
                tracing::warn!("Signal stream ended unexpectedly");
                let status = child.wait().await?;
                watcher_handle.abort();
                cleanup::cleanup_and_sync(
                    &watcher_handle,
                    &mut child,
                    &tracker,
                    &provider,
                    &project_path,
                    &waylog_dir,
                    Some(status),
                )
                .await?;
                // Propagate child process exit code
                if !status.success() {
                    let exit_code = status.code().unwrap_or(1);
                    return Err(WaylogError::ChildProcessFailed(exit_code));
                }
                return Ok(());
            }
        }
        // Child process exited normally
        status_result = child.wait() => {
            let status = status_result?;
            watcher_handle.abort();
            cleanup::cleanup_and_sync(
                &watcher_handle,
                &mut child,
                &tracker,
                &provider,
                &project_path,
                &waylog_dir,
                Some(status),
            )
            .await?;
            Some(status)
        }
    };

    // Handle exit status and propagate child process exit code
    if let Some(status) = exit_status {
        if !status.success() {
            tracing::warn!("{} exited with status: {:?}", provider.name(), status);
            // Get the exit code from the status
            let exit_code = status.code().unwrap_or(1);
            return Err(WaylogError::ChildProcessFailed(exit_code));
        }
    }

    tracing::info!(
        "Session complete. Chat history saved to: {}",
        waylog_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::base::{ChatMessage, ChatSession, MessageMetadata, MessageRole};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::Path;
    use tempfile::TempDir;
    use tokio::process::Command as TokioCommand;

    // Mock Provider for testing
    struct MockProvider {
        name: String,
        sessions: HashMap<PathBuf, ChatSession>,
        latest_session: Option<PathBuf>,
    }

    impl MockProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                sessions: HashMap::new(),
                latest_session: None,
            }
        }

        fn add_session(&mut self, path: PathBuf, session: ChatSession) {
            self.sessions.insert(path.clone(), session);
            self.latest_session = Some(path);
        }
    }

    #[async_trait]
    impl providers::base::Provider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn data_dir(&self) -> Result<PathBuf> {
            Ok(PathBuf::from("/tmp"))
        }

        fn session_dir(&self, _project_path: &Path) -> Result<PathBuf> {
            Ok(PathBuf::from("/tmp/sessions"))
        }

        async fn find_latest_session(&self, _project_path: &Path) -> Result<Option<PathBuf>> {
            Ok(self.latest_session.clone())
        }

        async fn parse_session(&self, file_path: &Path) -> Result<ChatSession> {
            self.sessions.get(file_path).cloned().ok_or_else(|| {
                crate::error::WaylogError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Session not found: {}", file_path.display()),
                ))
            })
        }

        async fn get_all_sessions(&self, _project_path: &Path) -> Result<Vec<PathBuf>> {
            Ok(self.sessions.keys().cloned().collect())
        }

        fn is_installed(&self) -> bool {
            true
        }

        fn command(&self) -> &str {
            "mock"
        }
    }

    fn create_test_session(session_id: &str, message_count: usize) -> ChatSession {
        let now = Utc::now();
        let mut messages = Vec::new();
        for i in 0..message_count {
            messages.push(ChatMessage {
                id: format!("msg-{}", i),
                timestamp: now,
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("Message {}", i),
                metadata: MessageMetadata::default(),
            });
        }

        ChatSession {
            session_id: session_id.to_string(),
            provider: "test".to_string(),
            project_path: PathBuf::from("/test/project"),
            started_at: now,
            updated_at: now,
            messages,
        }
    }

    #[tokio::test]
    async fn test_cleanup_and_sync_with_new_messages() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();
        let waylog_dir = utils::path::get_waylog_dir(&project_path);
        utils::path::ensure_dir_exists(&waylog_dir).unwrap();

        // Create mock provider with a session
        let mut mock_provider = MockProvider::new("test");
        let session_file = temp_dir.path().join("session.json");
        let session = create_test_session("session-1", 5);
        mock_provider.add_session(session_file.clone(), session.clone());
        let provider: Arc<dyn providers::base::Provider> = Arc::new(mock_provider);

        // Create tracker
        let tracker = Arc::new(
            session::SessionTracker::new(project_path.clone(), provider.clone())
                .await
                .unwrap(),
        );

        // Create a simple watcher handle (spawn a task that just waits)
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Create a simple child process (echo command that exits immediately)
        let mut child = TokioCommand::new("echo").arg("test").spawn().unwrap();

        // Wait for child to exit
        let _ = child.wait().await;

        // Call cleanup_and_sync
        let result = cleanup::cleanup_and_sync(
            &watcher_handle,
            &mut child,
            &tracker,
            &provider,
            &project_path,
            &waylog_dir,
            None,
        )
        .await;

        assert!(result.is_ok());

        // Verify that markdown file was created
        let markdown_files: Vec<_> = std::fs::read_dir(&waylog_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .collect();

        // Should have created a markdown file with the messages
        assert!(!markdown_files.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_and_sync_with_no_new_messages() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();
        let waylog_dir = utils::path::get_waylog_dir(&project_path);
        utils::path::ensure_dir_exists(&waylog_dir).unwrap();

        // Create mock provider with no latest session
        let mock_provider = MockProvider::new("test");
        let provider: Arc<dyn providers::base::Provider> = Arc::new(mock_provider);

        // Create tracker
        let tracker = Arc::new(
            session::SessionTracker::new(project_path.clone(), provider.clone())
                .await
                .unwrap(),
        );

        // Create watcher handle
        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // Create child process
        let mut child = TokioCommand::new("echo").arg("test").spawn().unwrap();
        let _ = child.wait().await;

        // Call cleanup_and_sync - should succeed even with no messages
        let result = cleanup::cleanup_and_sync(
            &watcher_handle,
            &mut child,
            &tracker,
            &provider,
            &project_path,
            &waylog_dir,
            None,
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_and_sync_handles_errors_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();
        let waylog_dir = utils::path::get_waylog_dir(&project_path);
        utils::path::ensure_dir_exists(&waylog_dir).unwrap();

        // Create mock provider that returns error for find_latest_session
        struct ErrorProvider;

        #[async_trait]
        impl providers::base::Provider for ErrorProvider {
            fn name(&self) -> &str {
                "error"
            }

            fn data_dir(&self) -> Result<PathBuf> {
                Ok(PathBuf::from("/tmp"))
            }

            fn session_dir(&self, _project_path: &Path) -> Result<PathBuf> {
                Ok(PathBuf::from("/tmp/sessions"))
            }

            async fn find_latest_session(&self, _project_path: &Path) -> Result<Option<PathBuf>> {
                Err(crate::error::WaylogError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Permission denied",
                )))
            }

            async fn parse_session(&self, _file_path: &Path) -> Result<ChatSession> {
                unreachable!()
            }

            async fn get_all_sessions(&self, _project_path: &Path) -> Result<Vec<PathBuf>> {
                Ok(vec![])
            }

            fn is_installed(&self) -> bool {
                true
            }

            fn command(&self) -> &str {
                "error"
            }
        }

        let provider: Arc<dyn providers::base::Provider> = Arc::new(ErrorProvider);
        let tracker = Arc::new(
            session::SessionTracker::new(project_path.clone(), provider.clone())
                .await
                .unwrap(),
        );

        let watcher_handle = tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        let mut child = TokioCommand::new("echo").arg("test").spawn().unwrap();
        let _ = child.wait().await;

        // Should not panic even when provider returns error
        let result = cleanup::cleanup_and_sync(
            &watcher_handle,
            &mut child,
            &tracker,
            &provider,
            &project_path,
            &waylog_dir,
            None,
        )
        .await;

        // Should succeed despite errors (errors are logged but don't stop cleanup)
        assert!(result.is_ok());
    }
}
