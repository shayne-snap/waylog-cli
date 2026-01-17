use crate::error::{Result, WaylogError};
use crate::providers::base::*;
use crate::utils::path;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    fn data_dir(&self) -> Result<PathBuf> {
        path::get_ai_data_dir("claude").map(|p| p.join("projects"))
    }

    fn session_dir(&self, project_path: &Path) -> Result<PathBuf> {
        let encoded = path::encode_path_claude(project_path);
        Ok(self.data_dir()?.join(encoded))
    }

    async fn find_latest_session(&self, project_path: &Path) -> Result<Option<PathBuf>> {
        let candidates = self.get_all_sessions(project_path).await?;
        Ok(candidates.into_iter().next())
    }

    async fn get_all_sessions(&self, project_path: &Path) -> Result<Vec<PathBuf>> {
        let session_dir = self.session_dir(project_path)?;

        if !session_dir.exists() {
            return Ok(Vec::new());
        }

        // Find all .jsonl files
        let mut entries = fs::read_dir(&session_dir).await?;
        let mut candidates = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                // Filter main sessions
                if self.is_main_session(&path).await.unwrap_or(false) {
                    let metadata = fs::metadata(&path).await?;
                    let modified = metadata.modified()?;
                    candidates.push((path, modified));
                }
            }
        }

        // Sort by modification time, newest first
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(candidates.into_iter().map(|(p, _)| p).collect())
    }

    async fn parse_session(&self, file_path: &Path) -> Result<ChatSession> {
        let file = fs::File::open(file_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut messages = Vec::new();
        let mut session_id = String::new();
        let mut started_at = Utc::now();
        let mut project_path = PathBuf::new();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            let event: ClaudeEvent = serde_json::from_str(&line).map_err(WaylogError::Json)?;

            // Extract session metadata from first event
            if session_id.is_empty() {
                session_id = event.session_id.clone().unwrap_or_else(|| {
                    file_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });

                if let Some(cwd) = &event.cwd {
                    project_path = PathBuf::from(cwd);
                }
            }

            // Parse user and assistant messages
            if event.event_type == "user" || event.event_type == "assistant" {
                if let Some(msg) = self.parse_message(event)? {
                    if messages.is_empty() {
                        started_at = msg.timestamp;
                    }
                    messages.push(msg);
                }
            }
        }

        Ok(ChatSession {
            session_id,
            provider: self.name().to_string(),
            project_path,
            started_at,
            updated_at: messages.last().map(|m| m.timestamp).unwrap_or(started_at),
            messages,
        })
    }

    fn is_installed(&self) -> bool {
        which::which("claude").is_ok()
    }

    fn command(&self) -> &str {
        "claude"
    }
}

impl ClaudeProvider {
    fn parse_message(&self, event: ClaudeEvent) -> Result<Option<ChatMessage>> {
        let role = match event.event_type.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            _ => return Ok(None),
        };

        // Extract content from message
        let content = match &event.message {
            Some(msg) => match &msg.content {
                ClaudeContent::Text(text) => text.clone(),
                ClaudeContent::Array(items) => items
                    .iter()
                    .filter_map(|item| {
                        if item.content_type == "text" {
                            item.text.clone()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            },
            None => return Ok(None),
        };

        if content.is_empty() {
            return Ok(None);
        }

        // Format XML content to look like official export
        let content = if role == MessageRole::User {
            // Filter out internal IDE state messages (ide_opened_file, ide_edit_file, etc.)
            // We use a regex to match ANY tag starting with <ide_ and ending with </ide_...>
            // If the message is purely these tags (whitespace allowed), we skip it.
            // If there is other content (user typed text), we keep the text.

            // Note: We create Regex here. In a high-throughput server we'd use OnceLock/lazy_static,
            // but for a CLI syncing tool this is acceptable (or we could move it to struct).
            // The (?s) flag enables dot matches newline (multi-line matching).
            let re = regex::Regex::new(r"(?s)<ide_[a-z_]+>.*?</ide_[a-z_]+>")
                .map_err(|e| WaylogError::Internal(e.to_string()))?;
            let clean_content = re.replace_all(&content, "").to_string();

            if clean_content.trim().is_empty() {
                // If nothing remains after removing tags, it was purely internal state -> Skip
                return Ok(None);
            }

            Self::format_claude_xml(clean_content.trim())
        } else {
            content
        };

        // Final check: if content became empty after formatting (and it's not a tool-use only message we want to keep?
        // Logic says we keep tool calls if they are robust, but here we just check text content string).
        // If content is empty/whitespace AND no tool calls, skip.
        // Wait, current logic for tool_calls extraction is BELOW this block.
        // We need to be careful. The original code extracted tool_calls LATER (lines 184).
        // But `content` variable here is just the text part.
        // If text content is empty, we might still want to return the message IF it has tool calls (which are extracted from `event.message`).
        // However, the text content `content` specifically refers to the `Text` part.
        // If `content` is empty here, we verify later?
        // Original code: `if content.is_empty() { return Ok(None); }` at line 157.
        // This suggests that if there is NO text content (even if there are tool calls in `Array`), it returns None?
        // Let's check line 140-153. It extracts text from Array.
        // If an Array has ONLY tool_use and no text, `content` string matches "" (joined empty strings).
        // So YES, the original logic filtered out messages with NO text even if they had tool use.
        // My filtering logic above maintains this: if `clean_content` is empty, we return `Ok(None)`.

        let timestamp = event
            .timestamp
            .and_then(|ts| DateTime::parse_from_rfc3339(&ts).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        // Extract metadata
        let (model, tokens, tool_calls) = if let Some(msg) = &event.message {
            let model = msg.model.clone();
            let tokens = msg.usage.as_ref().map(|u| TokenUsage {
                input: u.input_tokens,
                output: u.output_tokens,
                cached: u.cache_read_input_tokens.unwrap_or(0),
            });

            // Extract tool calls
            let tool_calls = if let ClaudeContent::Array(items) = &msg.content {
                items
                    .iter()
                    .filter(|item| item.content_type == "tool_use")
                    .filter_map(|item| item.name.clone())
                    .collect()
            } else {
                Vec::new()
            };

            (model, tokens, tool_calls)
        } else {
            (None, None, Vec::new())
        };

        Ok(Some(ChatMessage {
            id: event
                .uuid
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            timestamp,
            role,
            content,
            metadata: MessageMetadata {
                model,
                tokens,
                tool_calls,
                thoughts: Vec::new(),
            },
        }))
    }

    /// Format Claude Code XML tags into markdown-friendly text
    fn format_claude_xml(content: &str) -> String {
        // Handle Command Name: <command-name>cmd</command-name>
        if let Some(start) = content.find("<command-name>") {
            if let Some(end) = content[start..].find("</command-name>") {
                let cmd = &content[start + 14..start + end];

                // Only format if command starts with slash (e.g. /resume)
                // This preserves user input like "<command-name>My Custom Command</command-name>"
                if cmd.trim().starts_with('/') {
                    return format!("> {}", cmd.trim());
                }
            }
        }

        // Handle Stdout: <local-command-stdout>output</local-command-stdout>
        if let Some(start) = content.find("<local-command-stdout>") {
            if let Some(end) = content[start..].find("</local-command-stdout>") {
                let out = &content[start + 22..start + end];
                return format!("> ⎿ {}", out.trim());
            }
        }

        content.to_string()
    }

    /// Check if a session file is a main session (not a sidechain)
    async fn is_main_session(&self, path: &Path) -> Result<bool> {
        let file = fs::File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut checked_lines = 0;
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            // Limit checks to first 10 lines
            if checked_lines >= 10 {
                break;
            }
            checked_lines += 1;

            // Fast path: simple string check
            if line.contains("\"isSidechain\":true") {
                return Ok(false);
            }
            if line.contains("\"isSidechain\":false") {
                return Ok(true);
            }

            // Precise path: JSON parsing
            if let Ok(event) = serde_json::from_str::<ClaudeEvent>(&line) {
                if let Some(true) = event.is_sidechain {
                    return Ok(false);
                }
            }
        }

        // Default to true if not specified
        Ok(true)
    }
}

// Claude Code JSONL event structures
#[derive(Debug, Deserialize)]
struct ClaudeEvent {
    #[serde(rename = "type")]
    event_type: String,

    #[serde(rename = "sessionId")]
    session_id: Option<String>,

    cwd: Option<String>,
    timestamp: Option<String>,
    uuid: Option<String>,

    #[serde(rename = "isSidechain")]
    is_sidechain: Option<bool>,

    message: Option<ClaudeMessage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[allow(dead_code)]
    role: String,
    content: ClaudeContent,
    model: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ClaudeContent {
    Text(String),
    Array(Vec<ClaudeContentItem>),
}

#[derive(Debug, Deserialize)]
struct ClaudeContentItem {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    name: Option<String>, // For tool_use
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: u32,
    output_tokens: u32,
    cache_read_input_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::base::{MessageRole, Provider};

    // Helper to create a user message event with content
    fn create_user_event(content: &str) -> ClaudeEvent {
        ClaudeEvent {
            event_type: "user".to_string(),
            session_id: Some("test-session".to_string()),
            cwd: None,
            timestamp: None,
            uuid: None,
            is_sidechain: None,
            message: Some(ClaudeMessage {
                role: "user".to_string(),
                content: ClaudeContent::Text(content.to_string()),
                model: None,
                usage: None,
            }),
        }
    }

    #[test]
    fn test_ide_tag_filtering() {
        let provider = ClaudeProvider::new();

        // Case 1: Pure IDE tag message should be filtered out
        let content = "<ide_opened_file>some/path/file.txt</ide_opened_file>";
        let event = create_user_event(content);
        let result = provider.parse_message(event).unwrap();

        assert!(
            result.is_none(),
            "Pure IDE tag message should be filtered out"
        );

        // Case 2: Mixed content (User text + IDE tag)
        let content = "Check this file.\n<ide_opened_file>path/to/file</ide_opened_file>";
        let event = create_user_event(content);
        let result = provider.parse_message(event).unwrap();

        assert!(result.is_some());
        let msg = result.unwrap();
        assert_eq!(
            msg.content, "Check this file.",
            "Tag should be stripped from mixed content"
        );
    }
}
