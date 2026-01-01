mod cli;
mod commands;
mod config;
mod error;
mod exporter;
mod init;
mod logging;
mod output;
mod providers;
mod session;
pub mod synchronizer;
mod utils;
mod watcher;

use clap::Parser;
use cli::{Cli, Commands, OutputFormat};
use commands::{handle_pull, handle_run};
use error::Result;
use output::Output;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create output handler
    let mut output = Output::new(cli.quiet, matches!(cli.output, OutputFormat::Json));

    // 1. Resolve project root directory
    let (project_root, is_new_project) = init::resolve_project_root(&cli.command, &mut output)?;

    // 2. Setup logging
    logging::setup_logging(&project_root, cli.verbose)?;

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

    Ok(())
}
