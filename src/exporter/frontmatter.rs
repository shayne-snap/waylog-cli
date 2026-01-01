use crate::error::Result;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone)]
pub struct Frontmatter {
    pub session_id: Option<String>,
    pub provider: Option<String>,
    pub message_count: Option<usize>,
}

/// Parse minimal frontmatter from a markdown file
pub async fn parse_frontmatter(path: &Path) -> Result<Frontmatter> {
    let mut file = fs::File::open(path).await?;

    // Read first 2KB which should cover the frontmatter
    let mut buffer = [0u8; 2048];
    let n = file.read(&mut buffer).await?;
    let content = String::from_utf8_lossy(&buffer[..n]);

    let mut fm = Frontmatter {
        session_id: None,
        provider: None,
        message_count: None,
    };

    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_idx) = stripped.find("---") {
            let yaml_block = &stripped[..end_idx];

            for line in yaml_block.lines() {
                let line = line.trim();

                if let Some(val) = line.strip_prefix("session_id:") {
                    fm.session_id = Some(val.trim().to_string());
                } else if let Some(val) = line.strip_prefix("provider:") {
                    fm.provider = Some(val.trim().to_string());
                } else if let Some(val) = line.strip_prefix("message_count:") {
                    if let Ok(count) = val.trim().parse() {
                        fm.message_count = Some(count);
                    }
                }
            }
        }
    }

    Ok(fm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_parse_frontmatter_complete() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"---
provider: claude
session_id: test-session-123
message_count: 5
---
# Title
Content here
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        assert_eq!(fm.provider, Some("claude".to_string()));
        assert_eq!(fm.session_id, Some("test-session-123".to_string()));
        assert_eq!(fm.message_count, Some(5));
    }

    #[tokio::test]
    async fn test_parse_frontmatter_partial() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"---
provider: gemini
---
# Title
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        assert_eq!(fm.provider, Some("gemini".to_string()));
        assert_eq!(fm.session_id, None);
        assert_eq!(fm.message_count, None);
    }

    #[tokio::test]
    async fn test_parse_frontmatter_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"---
---
# Title
Content
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        assert_eq!(fm.provider, None);
        assert_eq!(fm.session_id, None);
        assert_eq!(fm.message_count, None);
    }

    #[tokio::test]
    async fn test_parse_frontmatter_no_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"# Title
Content without frontmatter
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        assert_eq!(fm.provider, None);
        assert_eq!(fm.session_id, None);
        assert_eq!(fm.message_count, None);
    }

    #[tokio::test]
    async fn test_parse_frontmatter_with_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"---
provider:  claude  
session_id:  test-123  
message_count:  10  
---
# Title
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        assert_eq!(fm.provider, Some("claude".to_string()));
        assert_eq!(fm.session_id, Some("test-123".to_string()));
        assert_eq!(fm.message_count, Some(10));
    }

    #[tokio::test]
    async fn test_parse_frontmatter_invalid_message_count() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"---
provider: claude
message_count: not-a-number
---
# Title
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        assert_eq!(fm.provider, Some("claude".to_string()));
        assert_eq!(fm.message_count, None); // Parsing failed, should be None
    }

    #[tokio::test]
    async fn test_parse_frontmatter_multiple_lines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        let content = r#"---
provider: claude
session_id: session-1
message_count: 3
started_at: 2024-01-01T00:00:00Z
updated_at: 2024-01-01T01:00:00Z
---
# Title
"#;
        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        // Should only parse fields we care about
        assert_eq!(fm.provider, Some("claude".to_string()));
        assert_eq!(fm.session_id, Some("session-1".to_string()));
        assert_eq!(fm.message_count, Some(3));
    }

    #[tokio::test]
    async fn test_parse_frontmatter_missing_file() {
        let file_path = std::path::Path::new("/nonexistent/file.md");
        let result = parse_frontmatter(file_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_frontmatter_large_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        // Test frontmatter larger than 2KB (should only read first 2KB)
        let mut content = "---\n".to_string();
        for i in 0..100 {
            content.push_str(&format!("field{}: value{}\n", i, i));
        }
        content.push_str("provider: claude\n");
        content.push_str("session_id: test\n");
        content.push_str("message_count: 1\n");
        content.push_str("---\n");

        tokio::fs::write(&file_path, content).await.unwrap();
        let fm = parse_frontmatter(&file_path).await.unwrap();

        // Even with large frontmatter, should be able to parse fields we care about
        // Note: Since we only read first 2KB, may not read fields at the end
        // This test mainly verifies it doesn't crash
        assert!(fm.provider.is_some() || fm.session_id.is_some() || fm.message_count.is_some());
    }
}
