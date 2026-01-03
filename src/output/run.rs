use super::Output;
use std::io::{self, Write};

impl Output {
    /// Print missing agent error
    pub fn missing_agent(&mut self) -> io::Result<()> {
        self.error("Missing required argument <AGENT>")?;
        writeln!(self.stderr(), "\nUsage: waylog run <AGENT> [ARGS]...\n")?;
        writeln!(self.stderr(), "Available agents:")?;
        for provider in crate::providers::list_providers() {
            writeln!(self.stderr(), "- {}", provider)?;
        }
        writeln!(self.stderr(), "\nExample:\n  waylog run claude")?;
        Ok(())
    }

    /// Print unknown agent error
    pub fn unknown_agent(&mut self, name: &str) -> io::Result<()> {
        self.error(format!("'{}' is not a recognized agent.", name))?;
        writeln!(self.stderr(), "\nAvailable agents:")?;
        for provider in crate::providers::list_providers() {
            writeln!(self.stderr(), "- {}", provider)?;
        }
        writeln!(self.stderr(), "\nDid you mean to run 'waylog pull'?")?;
        Ok(())
    }

    /// Print agent not installed error
    pub fn agent_not_installed(&mut self, command: &str) -> io::Result<()> {
        self.error(format!("{} is not installed or not in PATH", command))?;
        writeln!(
            self.stderr(),
            "Please install it first before using waylog."
        )?;
        Ok(())
    }
}
