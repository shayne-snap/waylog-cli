use crate::cli::Commands;
use crate::config::WAYLOG_DIR;
use crate::error::Result;
use crate::output::Output;
use std::path::PathBuf;

/// Resolve the project root directory based on the command being executed.
/// Returns (project_root, is_new_project)
pub fn resolve_project_root(command: &Commands, output: &mut Output) -> Result<(PathBuf, bool)> {
    let found_root = crate::utils::path::find_project_root();

    match command {
        Commands::Pull { .. } => match found_root {
            Some(root) => {
                output.found_tracking(&root)?;
                Ok((root, false))
            }
            None => {
                // Interactive prompt for initialization
                let current_dir = std::env::current_dir()?;
                let waylog_path = current_dir.join(WAYLOG_DIR);

                output.not_initialized()?;
                output.init_prompt(&waylog_path)?;

                if dialoguer::Confirm::new()
                    .default(true)
                    .show_default(true)
                    .interact()
                    .unwrap_or(false)
                {
                    Ok((current_dir, true))
                } else {
                    output.aborted()?;
                    std::process::exit(0);
                }
            }
        },
        Commands::Run { .. } => match found_root {
            Some(root) => Ok((root, false)),
            None => {
                // For 'run', if no project found, initialize in current dir
                let current = std::env::current_dir()?;
                Ok((current, true))
            }
        },
    }
}
