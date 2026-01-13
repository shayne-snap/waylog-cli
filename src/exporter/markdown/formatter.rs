use crate::providers::base::{ChatMessage, MessageRole};
use chrono::{DateTime, Utc};

/// Format a single message
pub(crate) fn format_message(message: &ChatMessage) -> String {
    let mut md = String::new();

    // Header with role and timestamp
    let role_emoji = match message.role {
        MessageRole::User => "👤",
        MessageRole::Assistant => "🤖",
        MessageRole::System => "⚙️",
    };

    let role_name = match message.role {
        MessageRole::User => "User",
        MessageRole::Assistant => "Assistant",
        MessageRole::System => "System",
    };

    md.push_str(&format!(
        "## {} {} ({})\n\n",
        role_emoji,
        role_name,
        format_datetime(&message.timestamp)
    ));

    // Content
    md.push_str(&message.content);
    md.push('\n');

    // Tool calls (Claude Code)
    if !message.metadata.tool_calls.is_empty() {
        md.push_str("\n**Tools Used:**\n");
        for tool in &message.metadata.tool_calls {
            md.push_str(&format!("- `{}`\n", tool));
        }
    }

    // Thoughts (Gemini)
    if !message.metadata.thoughts.is_empty() {
        md.push_str("\n<details>\n<summary>💭 Thoughts</summary>\n\n");
        for thought in &message.metadata.thoughts {
            md.push_str(&format!("- {}\n", thought));
        }
        md.push_str("\n</details>\n");
    }

    md
}

/// Extract a title from the first user message
pub(crate) fn extract_title(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .find(|m| matches!(m.role, MessageRole::User))
        .map(|m| {
            // Take first line or first 60 characters (char-boundary safe)
            let first_line = m.content.lines().next().unwrap_or("Untitled Session");
            let char_count = first_line.chars().count();
            if char_count > 60 {
                let truncated: String = first_line.chars().take(60).collect();
                format!("{}...", truncated)
            } else {
                first_line.to_string()
            }
        })
        .unwrap_or_else(|| "Untitled Session".to_string())
}

/// Format datetime in a human-readable way
pub(crate) fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::base::MessageMetadata;

    fn create_test_message(content: &str, role: MessageRole) -> ChatMessage {
        ChatMessage {
            id: "test-id".to_string(),
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    #[test]
    fn test_extract_title_short_english() {
        let messages = vec![create_test_message("Hello world", MessageRole::User)];
        let title = extract_title(&messages);
        assert_eq!(title, "Hello world");
    }

    #[test]
    fn test_extract_title_long_english() {
        let long_text =
            "This is a very long message that exceeds sixty characters and should be truncated";
        let messages = vec![create_test_message(long_text, MessageRole::User)];
        let title = extract_title(&messages);
        assert!(title.ends_with("..."));
        assert!(title.len() <= 63); // 60 chars + "..."
    }

    #[test]
    fn test_extract_title_short_chinese() {
        let messages = vec![create_test_message("你好世界", MessageRole::User)];
        let title = extract_title(&messages);
        assert_eq!(title, "你好世界");
    }

    #[test]
    fn test_extract_title_long_chinese() {
        let long_chinese =
            "把 pg_stateful.yaml 改写为 docker compose 可以运行的yaml，输出到 docker-compose.yaml";
        let messages = vec![create_test_message(long_chinese, MessageRole::User)];
        // This should not panic
        let title = extract_title(&messages);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_extract_title_mixed_long() {
        let mixed = "这是一个包含English和中文的very long message that should be truncated properly without panic";
        let messages = vec![create_test_message(mixed, MessageRole::User)];
        let title = extract_title(&messages);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_extract_title_multiline() {
        let multiline = "First line\nSecond line\nThird line";
        let messages = vec![create_test_message(multiline, MessageRole::User)];
        let title = extract_title(&messages);
        assert_eq!(title, "First line");
    }

    #[test]
    fn test_extract_title_empty_messages() {
        let messages: Vec<ChatMessage> = vec![];
        let title = extract_title(&messages);
        assert_eq!(title, "Untitled Session");
    }

    #[test]
    fn test_extract_title_no_user_messages() {
        let messages = vec![
            create_test_message("Assistant response", MessageRole::Assistant),
            create_test_message("System message", MessageRole::System),
        ];
        let title = extract_title(&messages);
        assert_eq!(title, "Untitled Session");
    }

    #[test]
    fn test_extract_title_exactly_60_chars() {
        let exactly_60 = "a".repeat(60);
        let messages = vec![create_test_message(&exactly_60, MessageRole::User)];
        let title = extract_title(&messages);
        assert_eq!(title, exactly_60);
        assert!(!title.ends_with("..."));
    }

    #[test]
    fn test_extract_title_with_emoji() {
        let with_emoji = "Hello 👋 this is a message with emoji 🎉 that might be long enough to truncate properly";
        let messages = vec![create_test_message(with_emoji, MessageRole::User)];
        let title = extract_title(&messages);
        // Should not panic on emoji boundaries
        assert!(title.len() > 0);
    }

    #[test]
    fn test_extract_title_finds_first_user_message() {
        let messages = vec![
            create_test_message("System init", MessageRole::System),
            create_test_message("First user message", MessageRole::User),
            create_test_message("Second user message", MessageRole::User),
        ];
        let title = extract_title(&messages);
        assert_eq!(title, "First user message");
    }
}
