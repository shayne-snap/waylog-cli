use crate::error::Result;
use crate::output::Output;
use crate::synchronizer::SyncStatus;
use crate::{providers, session, synchronizer};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;

pub async fn handle_pull(
    provider_name: Option<String>,
    force: bool,
    verbose: bool,
    project_path: PathBuf,
    output: &mut Output,
) -> Result<()> {
    output.pull_start(&project_path)?;

    // Filter providers
    let providers_to_sync = if let Some(name) = provider_name {
        vec![providers::get_provider(&name)?]
    } else {
        // Sync all known providers
        vec![
            providers::get_provider("claude")?,
            providers::get_provider("gemini")?,
            providers::get_provider("codex")?,
        ]
    };

    let mut total_synced = 0;
    let mut total_uptodate = 0;

    for provider in providers_to_sync {
        if !provider.is_installed() {
            debug!("Skipping {} (not installed)", provider.name());
            continue;
        }

        // Create session tracker and synchronizer
        let tracker =
            Arc::new(session::SessionTracker::new(project_path.clone(), provider.clone()).await?);
        let synchronizer = synchronizer::Synchronizer::new(
            provider.clone(),
            project_path.clone(),
            tracker.clone(),
        );

        match synchronizer.sync_all(force).await {
            Ok(results) => {
                // Print section header
                output.provider_header(provider.name(), results.len())?;

                let mut provider_uptodate = 0;
                let mut provider_synced = 0;
                let mut provider_skipped = 0;
                let mut _provider_failed = 0;

                for (path, status) in results {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    match status {
                        SyncStatus::Synced { new_messages } => {
                            output.synced(&filename, new_messages, verbose)?;
                            provider_synced += 1;
                        }
                        SyncStatus::UpToDate => {
                            output.up_to_date(&filename, verbose)?;
                            provider_uptodate += 1;
                        }
                        SyncStatus::Failed(e) => {
                            output.failed(&filename, &e.to_string())?;
                            _provider_failed += 1;
                        }
                        SyncStatus::Skipped => {
                            output.skipped(&filename, verbose)?;
                            provider_skipped += 1;
                        }
                    }
                }

                if !verbose {
                    output.summary_compact(provider_synced, provider_uptodate)?;
                }
                if verbose && provider_skipped > 0 {
                    output.skipped(&format!("{} sessions", provider_skipped), verbose)?;
                }

                total_synced += provider_synced;
                total_uptodate += provider_uptodate;
            }
            Err(e) => {
                tracing::error!("Failed to scan {}: {}", provider.name(), e);
            }
        }

        // Save state after each provider
        tracker.save_state().await?;
    }

    output.summary(total_synced, total_uptodate)?;

    Ok(())
}
