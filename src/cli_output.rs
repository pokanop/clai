//! TTY-styled output using [`console`]. Respects `NO_COLOR` and non-TTY (plain text).

use std::io::{self, IsTerminal, Write};

use console::Style;

use crate::interactive_mode::InteractiveExecutionMode;
use crate::policy::PolicyDecision;
use crate::presentation::{pre_run_lines, PreRunLine};
use crate::schema::CommandProposal;
use crate::tty::{use_color_for_stdout, use_color_for_stream};

/// One-line “waiting for cloud” hint (stderr, dim).
pub fn eprint_cloud_request_prelude() {
    if !io::stderr().is_terminal() {
        return;
    }
    if use_color_for_stream(std::io::stderr()) {
        eprintln!(
            "  {} {}",
            Style::new().dim().cyan().apply_to("···"),
            Style::new()
                .dim()
                .italic()
                .apply_to("requesting from cloud…")
        );
    } else {
        eprintln!("[requesting from cloud...]");
    }
}

/// stderr: dim streaming line for local model output (raw tokens) while generating.
pub fn eprint_model_stream_prelude() {
    if !io::stderr().is_terminal() {
        return;
    }
    if !use_color_for_stream(std::io::stderr()) {
        eprintln!("[composing...]");
        return;
    }
    eprintln!(
        "  {} {}",
        Style::new().dim().cyan().apply_to("···"),
        Style::new().dim().italic().apply_to("composing (stream)")
    );
}

/// One decoded token (or subpiece) for live feedback.
pub fn eprint_model_stream_piece(piece: &str) {
    if !io::stderr().is_terminal() {
        return;
    }
    if use_color_for_stream(std::io::stderr()) {
        let s = Style::new().dim().apply_to(piece);
        eprint!("{s}");
    } else {
        eprint!("{piece}");
    }
    let _ = std::io::stderr().flush();
}

pub fn eprint_model_stream_end() {
    if io::stderr().is_terminal() {
        eprintln!();
    }
}

#[must_use]
fn out_style() -> bool {
    io::stdout().is_terminal() && use_color_for_stdout()
}

/// Bold brand title for a section.
fn title(s: &str) -> String {
    if out_style() {
        Style::new().bold().cyan().apply_to(s).to_string()
    } else {
        s.to_string()
    }
}

/// Dim `key:` with bright `value` (indented).
fn key_value(key: &str, value: &str) {
    if out_style() {
        let k = Style::new().dim().apply_to(format!("{key}:"));
        let v = Style::new().white().apply_to(value);
        println!("  {k} {v}");
    } else {
        println!("{key}: {value}");
    }
}

/// Print a styled pre-run block from structured lines.
pub fn print_pre_run(proposal: &CommandProposal, decision: &PolicyDecision) {
    if !out_style() {
        println!(
            "{}",
            crate::presentation::format_pre_run_presentation(proposal, decision,)
        );
        return;
    }
    for line in pre_run_lines(proposal, decision) {
        match line {
            PreRunLine::SectionProposal => {
                println!();
                println!(
                    "  {} {}",
                    Style::new().bold().cyan().apply_to("▸"),
                    Style::new().bold().cyan().apply_to("Proposal")
                );
            }
            PreRunLine::SectionBlocked => {
                println!();
                let s = if out_style() {
                    format!(
                        "  {} {}",
                        Style::new().bold().red().apply_to("▸"),
                        Style::new().bold().red().apply_to("Proposal (blocked)")
                    )
                } else {
                    "  Proposal (blocked)".to_string()
                };
                println!("{s}");
            }
            PreRunLine::CommandLine {
                needs_shell,
                line,
                blocked,
            } => {
                if blocked {
                    key_value("Command", &line);
                } else if needs_shell {
                    key_value("Shell (needs_shell)", &line);
                } else {
                    key_value("Run", &line);
                }
            }
            PreRunLine::ShellRequestNote => {
                let t = if out_style() {
                    Style::new()
                        .dim()
                        .yellow()
                        .apply_to("  Note: shell execution requested (`needs_shell`)")
                        .to_string()
                } else {
                    "  Note: shell execution requested (`needs_shell`)".to_string()
                };
                println!("{t}");
            }
            PreRunLine::WorkingDir(p) => key_value("Cwd", &p),
            PreRunLine::Intent(s) => key_value("Why", &s),
            PreRunLine::Confidence(c) => key_value("Confidence", &c),
            PreRunLine::PolicyConfirm => {
                let t = if out_style() {
                    Style::new()
                        .yellow()
                        .apply_to("  Policy: extra confirmation required before this can run")
                        .to_string()
                } else {
                    "  Policy: extra confirmation required before this can run".to_string()
                };
                println!("{t}");
            }
            PreRunLine::Blocked { reason } => {
                let t = if out_style() {
                    format!("  {} {}", Style::new().red().apply_to("Blocked:"), reason)
                } else {
                    format!("  Blocked: {reason}")
                };
                println!("{t}");
            }
            PreRunLine::WontRun => {
                let t = if out_style() {
                    Style::new()
                        .dim()
                        .apply_to("  This command will not be run")
                        .to_string()
                } else {
                    "  This command will not be run".to_string()
                };
                println!("{t}");
            }
        }
    }
    println!();
}

fn session_start_plain(effective: InteractiveExecutionMode, source: &str, model_line: &str) {
    crate::tty::println_labeled(
        "clai",
        &format!(
            "interactive session — source={source}  mode={}  (EOF or `exit` to quit; `help` for built-ins; Ctrl+C cancels the current request)",
            effective.as_str()
        ),
        crate::tty::Severity::Info,
    );
    println!("{model_line}");
}

/// Interactive session header (banner + model).
pub fn print_session_start(effective: InteractiveExecutionMode, source: &str, model_line: &str) {
    if !out_style() {
        session_start_plain(effective, source, model_line);
        return;
    }
    let mode = effective.as_str();
    let banner = format!("clai · interactive · {source} · {mode}");
    println!();
    println!(
        "  {} {}",
        Style::new().bold().magenta().apply_to("▸"),
        Style::new().bold().white().apply_to(banner)
    );
    println!(
        "  {}",
        Style::new().dim().apply_to(
            "EOF or `exit` to quit · `help` for built-ins · Ctrl+C cancels the current request"
        )
    );
    let ml = if model_line.is_empty() {
        "(not set)"
    } else {
        model_line
    };
    println!(
        "  {} {}",
        Style::new().dim().italic().apply_to("model:"),
        Style::new().green().apply_to(ml)
    );
    println!();
}

/// Unstyled prompt line (so print before readline; cursor after `clai> `).
pub fn print_clai_prompt() {
    if out_style() {
        print!("{}", Style::new().bold().magenta().apply_to("clai> "));
    } else {
        print!("clai> ");
    }
    let _ = io::stdout().flush();
}

fn session_help_plain(effective: InteractiveExecutionMode) {
    println!(
        "\
Built-ins:
  help          Show this help
  exit, quit    End the session
  reload        Reload local GGUF from disk (local sessions only; `llama` feature)

Execution modes (effective this session: {}):
  dry-run       Ask before run; default is not to execute (config/CLI/env; see README)
  confirm       Prompt before each run (default when not legacy dry-run mapped)
  auto          Run after presentation (still honors sensitive policy confirm unless --yes)

Overrides: CLI > env > config > built-in default. Env: CLAI_INTERACTIVE__EXECUTION=dry-run|confirm|auto
Global flags: --interactive-mode, --yes (forces auto + policy auto-confirm), --cloud

Ctrl+C: cancels the current request; use exit/quit or EOF to leave the session.
",
        effective.as_str()
    );
}

/// Styled help for `help` in interactive session.
pub fn print_session_help_styled(effective: InteractiveExecutionMode) {
    if !out_style() {
        session_help_plain(effective);
        return;
    }
    println!();
    println!("{}", title("── Built-ins"));
    for (cmd, desc) in [
        ("help", "Show this help"),
        ("exit, quit", "End the session"),
        ("reload", "Reload local GGUF (local + `llama` feature)"),
    ] {
        println!(
            "  {}  {}",
            Style::new().yellow().apply_to(cmd),
            Style::new().dim().apply_to(desc)
        );
    }
    println!();
    println!("{}", title("── Execution mode (this session)"));
    let m = effective.as_str();
    println!("  {}{m}", Style::new().green().apply_to("● "));
    println!(
        "  {}  {}",
        Style::new().dim().apply_to("dry-run"),
        Style::new()
            .dim()
            .apply_to("Ask before run (default: skip)"),
    );
    println!(
        "  {}  {}",
        Style::new().dim().apply_to("confirm"),
        Style::new()
            .dim()
            .apply_to("Prompt before each run (default)"),
    );
    println!(
        "  {}  {}",
        Style::new().dim().apply_to("auto"),
        Style::new()
            .dim()
            .apply_to("Run after presentation (sensitive still confirms unless --yes)"),
    );
    println!();
    println!(
        "  {}",
        Style::new().dim().apply_to(
            "Config: CLAI_INTERACTIVE__EXECUTION · Global: --interactive-mode, --yes, --cloud"
        ),
    );
    println!();
}

/// Hint after dry-run in interactive.
pub fn print_dry_run_skip_note() {
    if out_style() {
        println!(
            "  {} {}",
            Style::new().cyan().apply_to("○"),
            Style::new()
                .dim()
                .italic()
                .apply_to("dry-run: not executed (preview only)")
        );
    } else {
        println!("(dry-run interactive mode; not executed)");
    }
}

/// Run preview line: `Run:` / non-direct (styled).
pub fn print_run_hint(line: &str) {
    if out_style() {
        println!("  {} {}", Style::new().bold().green().apply_to("▶"), line);
    } else {
        println!("{line}");
    }
}
