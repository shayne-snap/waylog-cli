use crate::error::Result;
use crate::session::state::SessionState;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Scan markdown files to restore session state
/// Returns a map of session_id -> SessionState
pub(crate) async fn restore_from_disk(
    project_dir: &std::path::Path,
    provider_name: &str,
) -> Result<HashMap<String, SessionState>> {
    let history_dir = crate::utils::path::get_waylog_dir(project_dir);
    if !history_dir.exists() {
        return Ok(HashMap::new());
    }

    // Read directory
    let mut entries = match fs::read_dir(&history_dir).await {
        Ok(e) => e,
        Err(_) => return Ok(HashMap::new()),
    };

    let mut sessions_map = HashMap::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            // Try to parse frontmatter
            if let Ok(fm) = crate::exporter::parse_frontmatter(&path).await {
                if let Some(sid) = fm.session_id {
                    let session_state = SessionState {
                        session_id: sid.clone(),
                        provider: fm.provider.unwrap_or_else(|| provider_name.to_string()),
                        file_path: PathBuf::new(), // Unknown source path
                        markdown_path: path.clone(),
                        synced_message_count: fm.message_count.unwrap_or(0),
                        last_sync_time: chrono::Utc::now(), // Unknown
                    };
                    sessions_map.insert(sid, session_state);
                }
            }
        }
    }

    Ok(sessions_map)
}
