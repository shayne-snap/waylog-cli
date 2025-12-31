use crate::error::{Result, WaylogError};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Get the home directory in a cross-platform way
pub fn home_dir() -> Result<PathBuf> {
    home::home_dir()
        .ok_or_else(|| WaylogError::PathError("Could not find home directory".to_string()))
}

/// Get the data directory for AI tools
/// On Unix: ~/.{tool}
/// On Windows: %USERPROFILE%\.{tool} (future extension point)
pub fn get_ai_data_dir(tool_name: &str) -> Result<PathBuf> {
    let home = home_dir()?;

    #[cfg(target_os = "windows")]
    {
        // Windows: Use AppData\Local for application data (future extension)
        // For now, keep it simple and use home directory
        Ok(home.join(format!(".{}", tool_name)))
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Unix-like systems (macOS, Linux)
        Ok(home.join(format!(".{}", tool_name)))
    }
}

/// Encode a path for Claude Code (replace / or \ with -)
/// Unix: /Users/name/project -> -Users-name-project
/// Windows: C:\Users\name\project -> C--Users-name-project
pub fn encode_path_claude(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    // Normalize path separators to forward slash first
    let normalized = path_str.replace('\\', "/");

    // Replace all slashes with hyphens (including the leading one)
    normalized.replace(['/', ':'], "-") // Handle Windows drive letters (C: -> C-)
}

/// Encode a path for Gemini (SHA-256 hash)
/// This is platform-independent as it hashes the string representation
/// Example: /Users/name/project -> f5ca4b7f107121b48048aa4ebe261a7ee63769dfc3a06e56191c987c8b51176d
pub fn encode_path_gemini(path: &Path) -> String {
    // Use the canonical string representation for consistent hashing
    let path_str = path.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Get the .waylog/history directory for the current project
pub fn get_waylog_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(".waylog").join("history")
}

/// Find the project root by looking for .waylog folder or .git folder
/// moving upwards from the current directory.
/// If we reach the home directory or the system root without finding a marker,
/// returns the current directory to avoid treat the whole home as a project.
pub fn find_project_root() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = home_dir().ok();

    for path in current_dir.ancestors() {
        if path.join(".waylog").is_dir() {
            return Some(path.to_path_buf());
        }

        // Stop if we've reached the user's home directory
        if let Some(ref home_path) = home {
            if path == home_path {
                break;
            }
        }
    }

    None
}

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_path_claude_unix() {
        let path = Path::new("/Users/goranka/Engineer/ai/helloai");
        assert_eq!(
            encode_path_claude(path),
            "-Users-goranka-Engineer-ai-helloai"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_encode_path_claude_windows() {
        let path = Path::new("C:\\Users\\goranka\\project");
        assert_eq!(encode_path_claude(path), "C--Users-goranka-project");
    }

    #[test]
    fn test_encode_path_gemini() {
        let path = Path::new("/Users/goranka/Engineer/ai/helloai");
        let hash = encode_path_gemini(path);
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex characters
        assert_eq!(
            hash,
            "f5ca4b7f107121b48048aa4ebe261a7ee63769dfc3a06e56191c987c8b51176d"
        );
    }

    #[test]
    fn test_get_ai_data_dir() {
        let dir = get_ai_data_dir("claude").unwrap();
        assert!(dir.to_string_lossy().contains(".claude"));
    }
}
