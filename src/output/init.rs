use super::Output;
use std::io::{self, Write};

impl Output {
    /// Print found tracking message
    pub fn found_tracking(&mut self, path: &std::path::Path) -> io::Result<()> {
        if !self.quiet() {
            if self.json() {
                self.print_json_internal("found_tracking", &path.display().to_string())?;
            } else {
                writeln!(
                    self.stdout(),
                    "Found existing tracking at: {}",
                    path.display()
                )?;
            }
        }
        Ok(())
    }

    /// Print not initialized message (interactive, always shown)
    pub fn not_initialized(&mut self) -> io::Result<()> {
        writeln!(self.stdout(), "Not initialized.")?;
        Ok(())
    }

    /// Print initialization prompt (interactive, always shown)
    pub fn init_prompt(&mut self, path: &std::path::Path) -> io::Result<()> {
        writeln!(
            self.stdout(),
            "Start tracking AI chat history in this directory?"
        )?;
        writeln!(self.stdout(), "Path: {}", path.display())?;
        Ok(())
    }

    /// Print aborted message (interactive, always shown)
    pub fn aborted(&mut self) -> io::Result<()> {
        writeln!(self.stdout(), "Aborted.")?;
        Ok(())
    }
}
