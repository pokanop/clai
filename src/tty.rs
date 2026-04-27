//! TTY-oriented message styling with `NO_COLOR` support (NFR-2, NFR-3, SC-3, SC-4).

use std::io::{self, IsTerminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Ok,
    Warn,
    Error,
}

#[must_use]
pub fn use_color_for_stream(stream: io::Stderr) -> bool {
    stream.is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

#[must_use]
pub fn use_color_for_stdout() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Prefix + optional ANSI color for stderr (session diagnostics).
pub fn format_labeled_line(label: &str, message: &str, sev: Severity, color: bool) -> String {
    let (open, close) = if color {
        match sev {
            Severity::Info => ("\x1b[36m", "\x1b[0m"),  // cyan
            Severity::Ok => ("\x1b[32m", "\x1b[0m"),    // green
            Severity::Warn => ("\x1b[33m", "\x1b[0m"),  // yellow
            Severity::Error => ("\x1b[31m", "\x1b[0m"), // red
        }
    } else {
        ("", "")
    };
    format!("{open}{label}:{close} {message}")
}

pub fn eprintln_labeled(label: &str, message: &str, sev: Severity) {
    let line = format_labeled_line(label, message, sev, use_color_for_stream(io::stderr()));
    eprintln!("{line}");
}

pub fn println_labeled(label: &str, message: &str, sev: Severity) {
    let line = format_labeled_line(label, message, sev, use_color_for_stdout());
    println!("{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_disables_ansi() {
        std::env::set_var("NO_COLOR", "1");
        let s = format_labeled_line("error", "x", Severity::Error, use_color_for_stdout());
        assert!(!s.contains("\x1b["));
        std::env::remove_var("NO_COLOR");
    }
}
