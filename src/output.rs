use console::Emoji;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/// Output handler for user-facing messages
/// Uses Write trait for flexibility and testability
pub struct Output {
    stdout: StandardStream,
    stderr: StandardStream,
    quiet: bool,
    json: bool,
}

impl Output {
    /// Create a new Output instance
    pub fn new(quiet: bool, json: bool) -> Self {
        let color_choice = if atty::is(atty::Stream::Stdout) {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        };

        Self {
            stdout: StandardStream::stdout(color_choice),
            stderr: StandardStream::stderr(color_choice),
            quiet,
            json,
        }
    }

    // ========== Basic Output Methods ==========

    /// Print an info message
    #[allow(dead_code)]
    pub fn info(&mut self, msg: impl AsRef<str>) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json("info", msg.as_ref())?;
            } else {
                writeln!(self.stdout, "{}", msg.as_ref())?;
            }
        }
        Ok(())
    }

    /// Print a success message (green)
    #[allow(dead_code)]
    pub fn success(&mut self, msg: impl AsRef<str>) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json("success", msg.as_ref())?;
            } else {
                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(self.stdout, "✓ {}", msg.as_ref())?;
                self.stdout.reset()?;
            }
        }
        Ok(())
    }

    /// Print an error message (red, always shown)
    pub fn error(&mut self, msg: impl AsRef<str>) -> io::Result<()> {
        if self.json {
            self.print_json("error", msg.as_ref())?;
        } else {
            self.stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
            writeln!(self.stderr, "✗ {}", msg.as_ref())?;
            self.stderr.reset()?;
        }
        Ok(())
    }

    /// Print a warning message (yellow)
    #[allow(dead_code)]
    pub fn warn(&mut self, msg: impl AsRef<str>) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json("warn", msg.as_ref())?;
            } else {
                self.stderr
                    .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
                writeln!(self.stderr, "⚠ {}", msg.as_ref())?;
                self.stderr.reset()?;
            }
        }
        Ok(())
    }

    // ========== Pull Command Specific ==========

    /// Print pull start message
    pub fn pull_start(&mut self, project_path: &std::path::Path) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json(
                    "pull_start",
                    &format!(
                        "Pulling chat history for project: {}",
                        project_path.display()
                    ),
                )?;
            } else {
                writeln!(
                    self.stdout,
                    "Pulling chat history for project: {}",
                    project_path.display()
                )?;
            }
        }
        Ok(())
    }

    /// Print provider section header
    pub fn provider_header(&mut self, provider: &str, count: usize) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json(
                    "provider_header",
                    &format!("{}: {} sessions", provider, count),
                )?;
            } else {
                writeln!(self.stdout, "\n[{}] Found {} sessions", provider, count)?;
            }
        }
        Ok(())
    }

    /// Print synced status (cyan)
    pub fn synced(&mut self, filename: &str, new_messages: usize, verbose: bool) -> io::Result<()> {
        if !self.quiet && verbose {
            if self.json {
                self.print_json(
                    "synced",
                    &format!("{}: {} new messages", filename, new_messages),
                )?;
            } else {
                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                writeln!(
                    self.stdout,
                    "  ↑ Synced: {} ({} new messages)",
                    filename, new_messages
                )?;
                self.stdout.reset()?;
            }
        }
        Ok(())
    }

    /// Print up-to-date status (green)
    pub fn up_to_date(&mut self, filename: &str, verbose: bool) -> io::Result<()> {
        if !self.quiet && verbose {
            if self.json {
                self.print_json("up_to_date", filename)?;
            } else {
                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(self.stdout, "  ✓ Up to date: {}", filename)?;
                self.stdout.reset()?;
            }
        }
        Ok(())
    }

    /// Print failed status (red, always shown)
    pub fn failed(&mut self, filename: &str, error: &str) -> io::Result<()> {
        if self.json {
            self.print_json("failed", &format!("{}: {}", filename, error))?;
        } else {
            self.stderr
                .set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
            writeln!(self.stderr, "  ✗ Failed to sync {}: {}", filename, error)?;
            self.stderr.reset()?;
        }
        Ok(())
    }

    /// Print skipped status (dim)
    pub fn skipped(&mut self, filename: &str, verbose: bool) -> io::Result<()> {
        if !self.quiet && verbose {
            if self.json {
                self.print_json("skipped", filename)?;
            } else {
                self.stdout.set_color(ColorSpec::new().set_intense(true))?;
                writeln!(
                    self.stdout,
                    "  ⊘ Skipped: {} (empty or invalid session)",
                    filename
                )?;
                self.stdout.reset()?;
            }
        }
        Ok(())
    }

    /// Print summary with emoji
    pub fn summary(&mut self, synced: usize, uptodate: usize) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json(
                    "summary",
                    &format!("{} synced, {} up to date", synced, uptodate),
                )?;
            } else {
                writeln!(
                    self.stdout,
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
        if !self.quiet {
            if synced > 0 {
                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                writeln!(self.stdout, "  ↑ {} sessions synced", synced)?;
                self.stdout.reset()?;
            }
            if uptodate > 0 {
                self.stdout
                    .set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(self.stdout, "  ✓ {} sessions up to date", uptodate)?;
                self.stdout.reset()?;
            }
        }
        Ok(())
    }

    // ========== Init Command Specific ==========

    /// Print found tracking message
    pub fn found_tracking(&mut self, path: &std::path::Path) -> io::Result<()> {
        if !self.quiet {
            if self.json {
                self.print_json("found_tracking", &path.display().to_string())?;
            } else {
                writeln!(
                    self.stdout,
                    "Found existing tracking at: {}",
                    path.display()
                )?;
            }
        }
        Ok(())
    }

    /// Print not initialized message (interactive, always shown)
    pub fn not_initialized(&mut self) -> io::Result<()> {
        writeln!(self.stdout, "Not initialized.")?;
        Ok(())
    }

    /// Print initialization prompt (interactive, always shown)
    pub fn init_prompt(&mut self, path: &std::path::Path) -> io::Result<()> {
        writeln!(
            self.stdout,
            "Start tracking AI chat history in this directory?"
        )?;
        writeln!(self.stdout, "Path: {}", path.display())?;
        Ok(())
    }

    /// Print aborted message (interactive, always shown)
    pub fn aborted(&mut self) -> io::Result<()> {
        writeln!(self.stdout, "Aborted.")?;
        Ok(())
    }

    // ========== Run Command Specific ==========

    /// Print missing agent error
    pub fn missing_agent(&mut self) -> io::Result<()> {
        self.error("Missing required argument <AGENT>")?;
        writeln!(self.stderr, "\nUsage: waylog run <AGENT> [ARGS]...\n")?;
        writeln!(self.stderr, "Available agents:")?;
        for provider in crate::providers::list_providers() {
            writeln!(self.stderr, "- {}", provider)?;
        }
        writeln!(self.stderr, "\nExample:\n  waylog run claude")?;
        Ok(())
    }

    /// Print unknown agent error
    pub fn unknown_agent(&mut self, name: &str) -> io::Result<()> {
        self.error(format!("'{}' is not a recognized agent.", name))?;
        writeln!(self.stderr, "\nAvailable agents:")?;
        for provider in crate::providers::list_providers() {
            writeln!(self.stderr, "- {}", provider)?;
        }
        writeln!(self.stderr, "\nDid you mean to run 'waylog pull'?")?;
        Ok(())
    }

    /// Print agent not installed error
    pub fn agent_not_installed(&mut self, command: &str) -> io::Result<()> {
        self.error(format!("{} is not installed or not in PATH", command))?;
        writeln!(self.stderr, "Please install it first before using waylog.")?;
        Ok(())
    }

    // ========== Progress Bar ==========

    /// Create a progress bar (returns None if quiet or json mode)
    #[allow(dead_code)]
    pub fn create_progress(&self, total: u64, message: &str) -> Option<ProgressBar> {
        if self.quiet || self.json {
            return None;
        }

        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.set_message(message.to_string());
        Some(pb)
    }

    // ========== JSON Output ==========

    fn print_json(&mut self, level: &str, message: &str) -> io::Result<()> {
        let json = serde_json::json!({
            "level": level,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        writeln!(self.stdout, "{}", json)?;
        Ok(())
    }
}
