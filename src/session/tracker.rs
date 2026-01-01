use crate::error::Result;
use crate::providers::base::{ChatSession, Provider};
use crate::session::state::{ProjectState, SessionState};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

/// Session tracker - manages active sessions and their sync state
pub struct SessionTracker {
    project_dir: PathBuf,
    provider: Arc<dyn Provider>,
    state: Arc<Mutex<ProjectState>>,
}

impl SessionTracker {
    /// Create a new session tracker
    /// Create a new session tracker
    pub async fn new(project_dir: PathBuf, provider: Arc<dyn Provider>) -> Result<Self> {
        // Start with empty state (stateless design)
        let state = ProjectState {
            sessions: std::collections::HashMap::new(),
        };

        let tracker = Self {
            project_dir,
            provider,
            state: Arc::new(Mutex::new(state)),
        };

        // Restore state from existing markdown files
        tracker.restore_from_disk().await?;

        Ok(tracker)
    }

    /// Get the current sync state
    pub async fn get_state(&self) -> ProjectState {
        self.state.lock().await.clone()
    }

    /// Save the current state to disk
    pub async fn save_state(&self) -> Result<()> {
        // Persistence disabled: Markdown files are the source of truth
        Ok(())
    }

    /// Get the number of synced messages for a session
    pub async fn get_synced_count(&self, session_id: &str) -> usize {
        let state = self.state.lock().await;
        state.get_synced_count(session_id)
    }

    /// Get the existing markdown path for a session if it exists
    pub async fn get_markdown_path(&self, session_id: &str) -> Option<PathBuf> {
        let state = self.state.lock().await;
        state
            .sessions
            .get(session_id)
            .map(|s| s.markdown_path.clone())
    }

    /// Update session state after syncing
    pub async fn update_session(
        &self,
        session_id: String,
        file_path: PathBuf,
        markdown_path: PathBuf,
        synced_count: usize,
    ) -> Result<()> {
        let mut state = self.state.lock().await;

        let session_state = SessionState {
            session_id: session_id.clone(),
            provider: self.provider.name().to_string(),
            file_path,
            markdown_path,
            synced_message_count: synced_count,
            last_sync_time: chrono::Utc::now(),
        };

        state.upsert_session(session_state);

        // Persistence disabled
        Ok(())
    }

    /// Process a session file and return new messages
    pub async fn get_new_messages(
        &self,
        file_path: &Path,
    ) -> Result<(ChatSession, Vec<crate::providers::base::ChatMessage>)> {
        // Parse the session
        let session = self.provider.parse_session(file_path).await?;

        // Get the number of already synced messages
        let synced_count = self.get_synced_count(&session.session_id).await;

        // Get new messages
        let new_messages = session
            .messages
            .iter()
            .skip(synced_count)
            .cloned()
            .collect();

        Ok((session, new_messages))
    }

    /// Scan markdown files to restore session state
    async fn restore_from_disk(&self) -> Result<()> {
        let history_dir = crate::utils::path::get_waylog_dir(&self.project_dir);
        if !history_dir.exists() {
            return Ok(());
        }

        // Read directory
        let mut entries = match fs::read_dir(&history_dir).await {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        let mut sessions_map = std::collections::HashMap::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                // Try to parse frontmatter
                if let Ok(fm) = crate::exporter::parse_frontmatter(&path).await {
                    if let Some(sid) = fm.session_id {
                        let state = SessionState {
                            session_id: sid.clone(),
                            provider: fm
                                .provider
                                .unwrap_or_else(|| self.provider.name().to_string()),
                            file_path: PathBuf::new(), // Unknown source path
                            markdown_path: path.clone(),
                            synced_message_count: fm.message_count.unwrap_or(0),
                            last_sync_time: chrono::Utc::now(), // Unknown
                        };
                        sessions_map.insert(sid, state);
                    }
                }
            }
        }

        // Update state if we found anything
        if !sessions_map.is_empty() {
            let mut state = self.state.lock().await;
            state.sessions = sessions_map;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::base::{ChatMessage, MessageMetadata, MessageRole};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::TempDir;

    // Mock Provider for testing
    struct MockProvider {
        name: String,
        sessions: HashMap<PathBuf, ChatSession>,
    }

    impl MockProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                sessions: HashMap::new(),
            }
        }

        fn add_session(&mut self, path: PathBuf, session: ChatSession) {
            self.sessions.insert(path, session);
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
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
            Ok(None)
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
    async fn test_new_tracker_empty_state() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        let state = tracker.get_state().await;
        assert_eq!(state.sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_new_tracker_restore_from_disk() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        tokio::fs::create_dir_all(&project_dir).await.unwrap();

        // Create .waylog/history directory (where markdown files are stored)
        let history_dir = crate::utils::path::get_waylog_dir(&project_dir);
        tokio::fs::create_dir_all(&history_dir).await.unwrap();

        // Create a markdown file with frontmatter
        let markdown_file = history_dir.join("session-123.md");
        let content = r#"---
provider: test
session_id: session-123
message_count: 5
---
# Session Content
"#;
        tokio::fs::write(&markdown_file, content).await.unwrap();

        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(project_dir, provider).await.unwrap();

        let state = tracker.get_state().await;
        assert_eq!(state.sessions.len(), 1);
        assert!(state.sessions.contains_key("session-123"));

        let session_state = state.sessions.get("session-123").unwrap();
        assert_eq!(session_state.session_id, "session-123");
        assert_eq!(session_state.synced_message_count, 5);
        assert_eq!(session_state.markdown_path, markdown_file);
    }

    #[tokio::test]
    async fn test_get_synced_count() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        // Initially no synced messages
        assert_eq!(tracker.get_synced_count("session-1").await, 0);

        // Update session
        tracker
            .update_session(
                "session-1".to_string(),
                PathBuf::from("/tmp/session-1.json"),
                PathBuf::from("/tmp/session-1.md"),
                10,
            )
            .await
            .unwrap();

        // Should return synced count
        assert_eq!(tracker.get_synced_count("session-1").await, 10);
    }

    #[tokio::test]
    async fn test_get_markdown_path() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        // Initially no markdown path
        assert_eq!(tracker.get_markdown_path("session-1").await, None);

        // Update session
        let markdown_path = PathBuf::from("/tmp/session-1.md");
        tracker
            .update_session(
                "session-1".to_string(),
                PathBuf::from("/tmp/session-1.json"),
                markdown_path.clone(),
                5,
            )
            .await
            .unwrap();

        // Should return markdown path
        assert_eq!(
            tracker.get_markdown_path("session-1").await,
            Some(markdown_path)
        );
    }

    #[tokio::test]
    async fn test_update_session() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        let session_id = "session-1".to_string();
        let file_path = PathBuf::from("/tmp/session-1.json");
        let markdown_path = PathBuf::from("/tmp/session-1.md");
        let synced_count = 7;

        tracker
            .update_session(
                session_id.clone(),
                file_path.clone(),
                markdown_path.clone(),
                synced_count,
            )
            .await
            .unwrap();

        let state = tracker.get_state().await;
        let session_state = state.sessions.get(&session_id).unwrap();

        assert_eq!(session_state.session_id, session_id);
        assert_eq!(session_state.provider, "test");
        assert_eq!(session_state.file_path, file_path);
        assert_eq!(session_state.markdown_path, markdown_path);
        assert_eq!(session_state.synced_message_count, synced_count);
    }

    #[tokio::test]
    async fn test_update_session_overwrites_existing() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        let session_id = "session-1".to_string();

        // First update
        tracker
            .update_session(
                session_id.clone(),
                PathBuf::from("/tmp/session-1.json"),
                PathBuf::from("/tmp/session-1.md"),
                5,
            )
            .await
            .unwrap();

        // Second update with different values
        tracker
            .update_session(
                session_id.clone(),
                PathBuf::from("/tmp/session-1-v2.json"),
                PathBuf::from("/tmp/session-1-v2.md"),
                10,
            )
            .await
            .unwrap();

        let state = tracker.get_state().await;
        assert_eq!(state.sessions.len(), 1);

        let session_state = state.sessions.get(&session_id).unwrap();
        assert_eq!(session_state.synced_message_count, 10);
        assert_eq!(
            session_state.markdown_path,
            PathBuf::from("/tmp/session-1-v2.md")
        );
    }

    #[tokio::test]
    async fn test_get_new_messages_no_existing_sync() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock_provider = MockProvider::new("test");

        let session_file = temp_dir.path().join("session.json");
        let session = create_test_session("session-1", 5);
        mock_provider.add_session(session_file.clone(), session.clone());

        let provider = Arc::new(mock_provider);
        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        let (parsed_session, new_messages) = tracker.get_new_messages(&session_file).await.unwrap();

        assert_eq!(parsed_session.session_id, "session-1");
        assert_eq!(new_messages.len(), 5); // All messages are new
        assert_eq!(new_messages[0].id, "msg-0");
        assert_eq!(new_messages[4].id, "msg-4");
    }

    #[tokio::test]
    async fn test_get_new_messages_with_existing_sync() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock_provider = MockProvider::new("test");

        let session_file = temp_dir.path().join("session.json");
        let session = create_test_session("session-1", 10);
        mock_provider.add_session(session_file.clone(), session.clone());

        let provider = Arc::new(mock_provider);
        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        // Mark first 3 messages as synced
        tracker
            .update_session(
                "session-1".to_string(),
                session_file.clone(),
                PathBuf::from("/tmp/session-1.md"),
                3,
            )
            .await
            .unwrap();

        let (parsed_session, new_messages) = tracker.get_new_messages(&session_file).await.unwrap();

        assert_eq!(parsed_session.session_id, "session-1");
        assert_eq!(new_messages.len(), 7); // Only last 7 messages are new
        assert_eq!(new_messages[0].id, "msg-3");
        assert_eq!(new_messages[6].id, "msg-9");
    }

    #[tokio::test]
    async fn test_get_new_messages_all_synced() {
        let temp_dir = TempDir::new().unwrap();
        let mut mock_provider = MockProvider::new("test");

        let session_file = temp_dir.path().join("session.json");
        let session = create_test_session("session-1", 5);
        mock_provider.add_session(session_file.clone(), session.clone());

        let provider = Arc::new(mock_provider);
        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        // Mark all messages as synced
        tracker
            .update_session(
                "session-1".to_string(),
                session_file.clone(),
                PathBuf::from("/tmp/session-1.md"),
                5,
            )
            .await
            .unwrap();

        let (parsed_session, new_messages) = tracker.get_new_messages(&session_file).await.unwrap();

        assert_eq!(parsed_session.session_id, "session-1");
        assert_eq!(new_messages.len(), 0); // No new messages
    }

    #[tokio::test]
    async fn test_restore_from_disk_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        tokio::fs::create_dir_all(&project_dir).await.unwrap();

        // Create .waylog/history directory (where markdown files are stored)
        let history_dir = crate::utils::path::get_waylog_dir(&project_dir);
        tokio::fs::create_dir_all(&history_dir).await.unwrap();

        // Create multiple markdown files
        let content1 = r#"---
provider: test
session_id: session-1
message_count: 3
---
# Session 1
"#;
        tokio::fs::write(history_dir.join("session-1.md"), content1)
            .await
            .unwrap();

        let content2 = r#"---
provider: test
session_id: session-2
message_count: 7
---
# Session 2
"#;
        tokio::fs::write(history_dir.join("session-2.md"), content2)
            .await
            .unwrap();

        // Create a file without frontmatter (should be ignored)
        tokio::fs::write(history_dir.join("no-frontmatter.md"), "# No frontmatter")
            .await
            .unwrap();

        // Create a non-md file (should be ignored)
        tokio::fs::write(history_dir.join("readme.txt"), "Not a markdown file")
            .await
            .unwrap();

        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(project_dir, provider).await.unwrap();

        let state = tracker.get_state().await;
        assert_eq!(state.sessions.len(), 2);
        assert!(state.sessions.contains_key("session-1"));
        assert!(state.sessions.contains_key("session-2"));

        assert_eq!(
            state
                .sessions
                .get("session-1")
                .unwrap()
                .synced_message_count,
            3
        );
        assert_eq!(
            state
                .sessions
                .get("session-2")
                .unwrap()
                .synced_message_count,
            7
        );
    }

    #[tokio::test]
    async fn test_restore_from_disk_no_waylog_dir() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        tokio::fs::create_dir_all(&project_dir).await.unwrap();

        // Don't create .waylog directory

        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(project_dir, provider).await.unwrap();

        let state = tracker.get_state().await;
        assert_eq!(state.sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_restore_from_disk_missing_provider_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        tokio::fs::create_dir_all(&project_dir).await.unwrap();

        // Create .waylog/history directory (where markdown files are stored)
        let history_dir = crate::utils::path::get_waylog_dir(&project_dir);
        tokio::fs::create_dir_all(&history_dir).await.unwrap();

        // Create markdown file without provider in frontmatter
        let content = r#"---
session_id: session-1
message_count: 5
---
# Session
"#;
        tokio::fs::write(history_dir.join("session-1.md"), content)
            .await
            .unwrap();

        let mock_provider = MockProvider::new("test-provider");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(project_dir, provider).await.unwrap();

        let state = tracker.get_state().await;
        assert_eq!(state.sessions.len(), 1);

        let session_state = state.sessions.get("session-1").unwrap();
        // Should fallback to provider name
        assert_eq!(session_state.provider, "test-provider");
    }

    #[tokio::test]
    async fn test_save_state() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        // save_state is currently a no-op, should always succeed
        let result = tracker.save_state().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_state() {
        let temp_dir = TempDir::new().unwrap();
        let mock_provider = MockProvider::new("test");
        let provider = Arc::new(mock_provider);

        let tracker = SessionTracker::new(temp_dir.path().to_path_buf(), provider)
            .await
            .unwrap();

        let state1 = tracker.get_state().await;
        assert_eq!(state1.sessions.len(), 0);

        // Update state
        tracker
            .update_session(
                "session-1".to_string(),
                PathBuf::from("/tmp/session-1.json"),
                PathBuf::from("/tmp/session-1.md"),
                5,
            )
            .await
            .unwrap();

        let state2 = tracker.get_state().await;
        assert_eq!(state2.sessions.len(), 1);
        assert!(state2.sessions.contains_key("session-1"));
    }
}
