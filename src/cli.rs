use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "waylog")]
#[command(about = "Automatically sync AI chat history from various CLI tools", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run an AI CLI tool and automatically sync its chat history
    Run {
        /// The AI tool to run (codex, claude, gemini)
        agent: Option<String>,

        /// Additional arguments to pass to the agent
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Pull chat history from providers
    Pull {
        /// Specific provider to pull (if not specified, pulls all)
        #[arg(short, long)]
        provider: Option<String>,

        /// Force re-pull even if up to date
        #[arg(short, long)]
        force: bool,
    },
}
