use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, IsTerminal, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub mod init;
pub mod pull;
pub mod run;

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
        let color_choice = if std::io::stdout().is_terminal() {
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

    // ========== Internal helpers for submodules ==========

    pub(crate) fn stdout(&mut self) -> &mut StandardStream {
        &mut self.stdout
    }

    pub(crate) fn stderr(&mut self) -> &mut StandardStream {
        &mut self.stderr
    }

    pub(crate) fn quiet(&self) -> bool {
        self.quiet
    }

    pub(crate) fn json(&self) -> bool {
        self.json
    }

    pub(crate) fn print_json_internal(&mut self, level: &str, message: &str) -> io::Result<()> {
        self.print_json(level, message)
    }
}
