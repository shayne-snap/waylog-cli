mod cli;
mod commands;
mod error;
mod exporter;
mod init;
mod output;
mod providers;
mod session;
pub mod synchronizer;
mod utils;
mod watcher;

use clap::Parser;
use cli::{Cli, Commands, OutputFormat};
use commands::{handle_pull, handle_run};
use error::WaylogError;
use output::Output;
use std::io::Write;

#[tokio::main]
async fn main() {
    // Setup panic handler for user-friendly error messages
    human_panic::setup_panic!();

    let cli = Cli::parse();

    // Create output handler
    let mut output = Output::new(cli.quiet, matches!(cli.output, OutputFormat::Json));

    // Execute main logic and handle errors with appropriate exit codes
    let result = async {
        // 0. Validate provider for pull command BEFORE resolving project root
        // This ensures we catch invalid providers even if project is not initialized
        if let Commands::Pull {
            provider: Some(ref provider_name),
            ..
        } = cli.command
        {
            match providers::get_provider(provider_name) {
                Ok(_) => {} // Provider is valid, continue
                Err(WaylogError::ProviderNotFound(ref name)) => {
                    output.error(format!("'{}' is not a recognized provider.", name))?;
                    writeln!(output.stderr(), "\nAvailable providers:")?;
                    for provider in providers::list_providers() {
                        writeln!(output.stderr(), "- {}", provider)?;
                    }
                    return Err(WaylogError::ProviderNotFound(name.clone()));
                }
                Err(e) => return Err(e),
            }
        }

        // 1. Resolve project root directory
        let (project_root, is_new_project) = init::resolve_project_root(&cli.command, &mut output)?;

        // 2. Setup logging (only creates log file if verbose)
        init::setup_logging(&project_root, cli.verbose, cli.quiet)?;

        // 3. Log new project initialization if needed
        if is_new_project {
            tracing::info!(
                "Initializing new waylog project in: {}",
                project_root.display()
            );
        }

        // 4. Dispatch command
        match cli.command {
            Commands::Run { agent, args } => {
                handle_run(agent, args, project_root, &mut output).await?;
            }
            Commands::Pull { provider, force } => {
                handle_pull(provider, force, cli.verbose, project_root, &mut output).await?;
            }
        }

        Ok::<(), WaylogError>(())
    }
    .await;

    // Handle errors and exit with appropriate code
    match result {
        Ok(()) => std::process::exit(exitcode::OK),
        Err(e) => {
            // Display error message to user if not already shown
            // Some errors (like MissingAgent, ProviderNotFound, AgentNotInstalled) are
            // already displayed via output.error() in command handlers
            if !e.is_already_displayed() {
                let error_msg = format!("{}", e);
                let _ = output.error(&error_msg);
            }
            std::process::exit(e.exit_code());
        }
    }
}
