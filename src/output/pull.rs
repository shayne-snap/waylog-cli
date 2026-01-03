use super::Output;
use console::Emoji;
use std::io::{self, Write};
use termcolor::{Color, ColorSpec, WriteColor};

impl Output {
    /// Print unknown provider error
    pub fn unknown_provider(&mut self, name: &str) -> io::Result<()> {
        self.error(format!("'{}' is not a recognized provider.", name))?;
        writeln!(self.stderr(), "\nAvailable providers:")?;
        for provider in crate::providers::list_providers() {
            writeln!(self.stderr(), "- {}", provider)?;
        }
        Ok(())
    }
    /// Print pull start message
    pub fn pull_start(&mut self, project_path: &std::path::Path) -> io::Result<()> {
        if !self.quiet() {
            if self.json() {
                self.print_json_internal(
                    "pull_start",
                    &format!(
                        "Pulling chat history for project: {}",
                        project_path.display()
                    ),
                )?;
            } else {
                writeln!(
                    self.stdout(),
                    "Pulling chat history for project: {}",
                    project_path.display()
                )?;
            }
        }
        Ok(())
    }

    /// Print provider section header
    pub fn provider_header(&mut self, provider: &str, count: usize) -> io::Result<()> {
        if !self.quiet() {
            if self.json() {
                self.print_json_internal(
                    "provider_header",
                    &format!("{}: {} sessions", provider, count),
                )?;
            } else {
                writeln!(self.stdout(), "\n[{}] Found {} sessions", provider, count)?;
            }
        }
        Ok(())
    }

    /// Print synced status (cyan)
    pub fn synced(&mut self, filename: &str, new_messages: usize, verbose: bool) -> io::Result<()> {
        if !self.quiet() && verbose {
            if self.json() {
                self.print_json_internal(
                    "synced",
                    &format!("{}: {} new messages", filename, new_messages),
                )?;
            } else {
                self.stdout()
                    .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                writeln!(
                    self.stdout(),
                    "  ↑ Synced: {} ({} new messages)",
                    filename,
                    new_messages
                )?;
                self.stdout().reset()?;
            }
        }
        Ok(())
    }

    /// Print up-to-date status (green)
    pub fn up_to_date(&mut self, filename: &str, verbose: bool) -> io::Result<()> {
        if !self.quiet() && verbose {
            if self.json() {
                self.print_json_internal("up_to_date", filename)?;
            } else {
                self.stdout()
                    .set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(self.stdout(), "  ✓ Up to date: {}", filename)?;
                self.stdout().reset()?;
            }
        }
        Ok(())
    }

    /// Print failed status (red, always shown)
    pub fn failed(&mut self, filename: &str, error: &str) -> io::Result<()> {
        if self.json() {
            self.print_json_internal("failed", &format!("{}: {}", filename, error))?;
        } else {
            self.stderr()
                .set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
            writeln!(self.stderr(), "  ✗ Failed to sync {}: {}", filename, error)?;
            self.stderr().reset()?;
        }
        Ok(())
    }

    /// Print skipped status (dim)
    pub fn skipped(&mut self, filename: &str, verbose: bool) -> io::Result<()> {
        if !self.quiet() && verbose {
            if self.json() {
                self.print_json_internal("skipped", filename)?;
            } else {
                self.stdout()
                    .set_color(ColorSpec::new().set_intense(true))?;
                writeln!(
                    self.stdout(),
                    "  ⊘ Skipped: {} (empty or invalid session)",
                    filename
                )?;
                self.stdout().reset()?;
            }
        }
        Ok(())
    }

    /// Print summary with emoji
    pub fn summary(&mut self, synced: usize, uptodate: usize) -> io::Result<()> {
        if !self.quiet() {
            if self.json() {
                self.print_json_internal(
                    "summary",
                    &format!("{} synced, {} up to date", synced, uptodate),
                )?;
            } else {
                writeln!(
                    self.stdout(),
                    "\n{} Pull complete! {} sessions updated, {} up to date.",
                    Emoji("✨", ""),
                    synced,
                    uptodate
                )?;
            }
        }
        Ok(())
    }

    /// Print compact summary (non-verbose mode)
    pub fn summary_compact(&mut self, synced: usize, uptodate: usize) -> io::Result<()> {
        if !self.quiet() {
            if synced > 0 {
                self.stdout()
                    .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                writeln!(self.stdout(), "  ↑ {} sessions synced", synced)?;
                self.stdout().reset()?;
            }
            if uptodate > 0 {
                self.stdout()
                    .set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(self.stdout(), "  ✓ {} sessions up to date", uptodate)?;
                self.stdout().reset()?;
            }
        }
        Ok(())
    }
}
