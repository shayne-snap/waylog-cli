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
            // Take first line or first 60 characters
            let first_line = m.content.lines().next().unwrap_or("Untitled Session");
            if first_line.len() > 60 {
                format!("{}...", &first_line[..60])
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
