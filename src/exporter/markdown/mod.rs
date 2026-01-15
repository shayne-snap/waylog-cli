mod formatter;

use crate::error::Result;
use crate::providers::base::{ChatMessage, ChatSession};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Generate markdown content from a chat session
pub fn generate_markdown(session: &ChatSession) -> String {
    let mut md = String::new();

    // Frontmatter
    md.push_str("---\n");
    md.push_str(&format!("provider: {}\n", session.provider));
    md.push_str(&format!("session_id: {}\n", session.session_id));
    md.push_str(&format!("project: {}\n", session.project_path.display()));
    md.push_str(&format!(
        "started_at: {}\n",
        session.started_at.to_rfc3339()
    ));
    md.push_str(&format!(
        "updated_at: {}\n",
        session.updated_at.to_rfc3339()
    ));
    md.push_str(&format!("message_count: {}\n", session.messages.len()));

    // Calculate total tokens if available
    let total_tokens: u32 = session
        .messages
        .iter()
        .filter_map(|m| m.metadata.tokens.as_ref())
        .map(|t| t.input + t.output)
        .sum();

    if total_tokens > 0 {
        md.push_str(&format!("total_tokens: {}\n", total_tokens));
    }

    md.push_str("---\n\n");

    // Title
    let title = formatter::extract_title(&session.messages);
    md.push_str(&format!("# {}\n\n", title));

    // Messages
    for message in &session.messages {
        md.push_str(&formatter::format_message(message));
        md.push_str("\n\n");
    }

    md
}

/// Append new messages to an existing markdown file
pub async fn append_messages(file_path: &Path, messages: &[ChatMessage]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .await?;

    for message in messages {
        let content = formatter::format_message(message);
        file.write_all(content.as_bytes()).await?;
        file.write_all(b"\n\n").await?;
    }

    file.flush().await?;
    Ok(())
}

/// Create a new markdown file with the full session
pub async fn create_markdown_file(file_path: &Path, session: &ChatSession) -> Result<()> {
    let content = generate_markdown(session);
    fs::write(file_path, content).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::base::{MessageRole, TokenUsage};
    use chrono::Utc;
    use tempfile::TempDir;

    fn create_test_message(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            id: "1".to_string(),
            timestamp: Utc::now(),
            role,
            content: content.to_string(),
            metadata: Default::default(),
        }
    }

    fn create_test_session(messages: Vec<ChatMessage>) -> ChatSession {
        let now = Utc::now();
        ChatSession {
            session_id: "test-session".to_string(),
            provider: "claude".to_string(),
            project_path: std::env::temp_dir().join("test-project"),
            started_at: now,
            updated_at: now,
            messages,
        }
    }

    // extract_title tests
    #[test]
    fn test_extract_title() {
        let messages = vec![create_test_message(
            MessageRole::User,
            "How do I implement a CLI tool?",
        )];

        assert_eq!(
            formatter::extract_title(&messages),
            "How do I implement a CLI tool?"
        );
    }

    #[test]
    fn test_extract_title_long() {
        let messages = vec![create_test_message(
            MessageRole::User,
            "This is a very long message that should be truncated because it exceeds the maximum length",
        )];

        let title = formatter::extract_title(&messages);
        assert!(title.len() <= 63); // 60 + "..."
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_extract_title_no_user_message() {
        let messages = vec![
            create_test_message(MessageRole::System, "System message"),
            create_test_message(MessageRole::Assistant, "Assistant response"),
        ];

        assert_eq!(formatter::extract_title(&messages), "Untitled Session");
    }

    #[test]
    fn test_extract_title_empty_messages() {
        let messages = vec![];
        assert_eq!(formatter::extract_title(&messages), "Untitled Session");
    }

    #[test]
    fn test_extract_title_multiline_first_line() {
        let messages = vec![create_test_message(
            MessageRole::User,
            "First line\nSecond line\nThird line",
        )];

        assert_eq!(formatter::extract_title(&messages), "First line");
    }

    #[test]
    fn test_extract_title_empty_content() {
        let messages = vec![create_test_message(MessageRole::User, "")];
        assert_eq!(formatter::extract_title(&messages), "Untitled Session");
    }

    // format_datetime tests
    #[test]
    fn test_format_datetime() {
        use chrono::DateTime;
        let dt = DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let formatted = formatter::format_datetime(&dt);
        assert_eq!(formatted, "2024-01-01 12:00:00 UTC");
    }

    // format_message tests
    #[test]
    fn test_format_message_user() {
        let message = create_test_message(MessageRole::User, "Hello, world!");
        let formatted = formatter::format_message(&message);
        assert!(formatted.contains("👤"));
        assert!(formatted.contains("User"));
        assert!(formatted.contains("Hello, world!"));
    }

    #[test]
    fn test_format_message_assistant() {
        let message = create_test_message(MessageRole::Assistant, "Hello! How can I help?");
        let formatted = formatter::format_message(&message);
        assert!(formatted.contains("🤖"));
        assert!(formatted.contains("Assistant"));
        assert!(formatted.contains("Hello! How can I help?"));
    }

    #[test]
    fn test_format_message_system() {
        let message = create_test_message(MessageRole::System, "System prompt");
        let formatted = formatter::format_message(&message);
        assert!(formatted.contains("⚙️"));
        assert!(formatted.contains("System"));
        assert!(formatted.contains("System prompt"));
    }

    #[test]
    fn test_format_message_with_tool_calls() {
        let mut message = create_test_message(MessageRole::Assistant, "I'll use some tools");
        message.metadata.tool_calls = vec!["read_file".to_string(), "write_file".to_string()];
        let formatted = formatter::format_message(&message);
        assert!(formatted.contains("**Tools Used:**"));
        assert!(formatted.contains("`read_file`"));
        assert!(formatted.contains("`write_file`"));
    }

    #[test]
    fn test_format_message_with_thoughts() {
        let mut message = create_test_message(MessageRole::Assistant, "Response");
        message.metadata.thoughts = vec!["Thought 1".to_string(), "Thought 2".to_string()];
        let formatted = formatter::format_message(&message);
        assert!(formatted.contains("<details>"));
        assert!(formatted.contains("<summary>💭 Thoughts</summary>"));
        assert!(formatted.contains("Thought 1"));
        assert!(formatted.contains("Thought 2"));
    }

    #[test]
    fn test_format_message_multiline_content() {
        let message = create_test_message(MessageRole::User, "Line 1\nLine 2\nLine 3");
        let formatted = formatter::format_message(&message);
        assert!(formatted.contains("Line 1"));
        assert!(formatted.contains("Line 2"));
        assert!(formatted.contains("Line 3"));
    }

    // generate_markdown tests
    #[test]
    fn test_generate_markdown_basic() {
        let messages = vec![
            create_test_message(MessageRole::User, "Hello"),
            create_test_message(MessageRole::Assistant, "Hi there!"),
        ];
        let session = create_test_session(messages);
        let md = generate_markdown(&session);

        assert!(md.contains("provider: claude"));
        assert!(md.contains("session_id: test-session"));
        assert!(md.contains("message_count: 2"));
        assert!(md.contains("# Hello"));
        assert!(md.contains("Hello"));
        assert!(md.contains("Hi there!"));
    }

    #[test]
    fn test_generate_markdown_with_tokens() {
        let mut message = create_test_message(MessageRole::User, "Test");
        message.metadata.tokens = Some(TokenUsage {
            input: 10,
            output: 20,
            cached: 5,
        });
        let session = create_test_session(vec![message]);
        let md = generate_markdown(&session);

        assert!(md.contains("total_tokens: 30")); // 10 + 20
    }

    #[test]
    fn test_generate_markdown_without_tokens() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let session = create_test_session(messages);
        let md = generate_markdown(&session);

        assert!(!md.contains("total_tokens"));
    }

    #[test]
    fn test_generate_markdown_empty_messages() {
        let session = create_test_session(vec![]);
        let md = generate_markdown(&session);

        assert!(md.contains("message_count: 0"));
        assert!(md.contains("# Untitled Session"));
    }

    #[test]
    fn test_generate_markdown_multiple_messages() {
        let messages = vec![
            create_test_message(MessageRole::User, "Question 1"),
            create_test_message(MessageRole::Assistant, "Answer 1"),
            create_test_message(MessageRole::User, "Question 2"),
            create_test_message(MessageRole::Assistant, "Answer 2"),
        ];
        let session = create_test_session(messages);
        let md = generate_markdown(&session);

        assert!(md.contains("message_count: 4"));
        assert!(md.contains("Question 1"));
        assert!(md.contains("Answer 1"));
        assert!(md.contains("Question 2"));
        assert!(md.contains("Answer 2"));
    }

    #[test]
    fn test_generate_markdown_frontmatter_format() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let session = create_test_session(messages);
        let md = generate_markdown(&session);

        // Check frontmatter format
        assert!(md.starts_with("---\n"));
        assert!(md.contains("---\n\n")); // Frontmatter end
        assert!(md.contains("started_at:"));
        assert!(md.contains("updated_at:"));
    }

    // Async function tests
    #[tokio::test]
    async fn test_create_markdown_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");

        let messages = vec![
            create_test_message(MessageRole::User, "Hello"),
            create_test_message(MessageRole::Assistant, "Hi!"),
        ];
        let session = create_test_session(messages);

        create_markdown_file(&file_path, &session).await.unwrap();

        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("Hello"));
        assert!(content.contains("Hi!"));
    }

    #[tokio::test]
    async fn test_append_messages() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");

        // Create file first
        let initial_messages = vec![create_test_message(MessageRole::User, "First message")];
        let initial_session = create_test_session(initial_messages);
        create_markdown_file(&file_path, &initial_session)
            .await
            .unwrap();

        // Append new messages
        let new_messages = vec![create_test_message(
            MessageRole::Assistant,
            "Second message",
        )];
        append_messages(&file_path, &new_messages).await.unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("First message"));
        assert!(content.contains("Second message"));
    }

    #[tokio::test]
    async fn test_append_messages_to_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new.md");

        // Append to non-existent file
        let messages = vec![create_test_message(MessageRole::User, "New message")];
        append_messages(&file_path, &messages).await.unwrap();

        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("New message"));
    }
}
