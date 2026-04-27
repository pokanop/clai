//! TTY-styled output using [`console`]. Respects `NO_COLOR` and non-TTY (plain text).

use std::io::{self, IsTerminal, Write};

use console::Style;

use crate::config::ExecutionMode;
use crate::host_context::HostContext;
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

/// Primary line for a subcommand (matches `clai doctor` / `clai models · …` style).
pub fn cli_intro(command: &str, subtitle: &str) {
    println!();
    if out_style() {
        println!(
            "  {} {}",
            Style::new().bold().white().apply_to(command),
            Style::new().dim().apply_to(format!("— {subtitle}"))
        );
    } else {
        println!("{command} — {subtitle}");
    }
}

/// Section heading: `── Title` (cyan when color is enabled).
pub fn cli_section(title_tail: &str) {
    println!();
    println!("{}", title(&format!("── {title_tail}")));
}

/// Dim explanatory line, indented (footnotes, hints).
pub fn cli_note(line: &str) {
    if out_style() {
        println!("  {}", Style::new().dim().italic().apply_to(line));
    } else {
        println!("  {line}");
    }
}

/// Dim `key:` with bright `value` (indented). Used across doctor, models, init, and pre-run.
pub fn cli_kv(key: &str, value: &str) {
    if out_style() {
        let k = Style::new().dim().apply_to(format!("{key}:"));
        let v = Style::new().white().apply_to(value);
        println!("  {k} {v}");
    } else {
        println!("{key}: {value}");
    }
}

/// Pretty-print proposal JSON (`--print-only`, `--verbose`, interactive verbose).
pub fn print_proposal_json(proposal: &CommandProposal) -> crate::error::Result<()> {
    let pretty = serde_json::to_string_pretty(proposal).map_err(crate::error::AppError::Json)?;
    cli_section("Proposal (JSON)");
    println!("{pretty}");
    Ok(())
}

/// Verbose post-run block (`clai ask -v`, interactive verbose).
pub fn print_verbose_run_report(
    wrapper_context: Option<&str>,
    status_line: &str,
    stdout: &str,
    stderr: &str,
) {
    if let Some(ctx) = wrapper_context {
        if !ctx.trim().is_empty() {
            cli_section("Execution context");
            println!("{ctx}");
        }
    }
    cli_section("Exit status");
    println!("{status_line}");
    cli_section("Stdout");
    if stdout.is_empty() {
        cli_note("(empty)");
    } else {
        println!("{stdout}");
    }
    cli_section("Stderr");
    if stderr.is_empty() {
        cli_note("(empty)");
    } else {
        println!("{stderr}");
    }
    println!();
}

/// stderr: encoding hint after captured streams (replacement char present).
pub fn eprint_captured_stream_encoding_note() {
    if !use_color_for_stream(io::stderr()) {
        eprintln!("note: captured output contained non-UTF-8 bytes (shown as U+FFFD).");
        return;
    }
    eprintln!(
        "  {}",
        Style::new().dim().italic().apply_to(
            "note: captured output contained non-UTF-8 bytes (shown as U+FFFD)."
        )
    );
}

/// One row for `clai models list`.
pub struct ModelCatalogRow {
    pub id: String,
    pub display_name: String,
    pub profile: String,
    pub location: String,
    pub is_default: bool,
}

pub fn print_models_list(rows: &[ModelCatalogRow]) {
    cli_intro("clai models · list", "registry entries and local files");
    cli_section("Catalog");
    if rows.is_empty() {
        cli_note("No models in the merged registry.");
        println!();
        return;
    }
    for r in rows {
        let header = if r.is_default {
            format!("* {}  (default)", r.id)
        } else {
            format!("  {}", r.id)
        };
        if out_style() {
            println!("  {}", Style::new().bold().white().apply_to(header));
        } else {
            println!("  {header}");
        }
        cli_kv("Name", &r.display_name);
        cli_kv("Profile", &r.profile);
        cli_kv("Location", &r.location);
        println!();
    }
}

pub fn print_models_search(query: &str, hits: &[(&str, &str)]) {
    cli_intro("clai models · search", query);
    cli_section("Matches");
    if hits.is_empty() {
        cli_note("No matching models.");
    } else {
        for (id, name) in hits {
            cli_kv(id, name);
        }
    }
    println!();
}

pub fn print_models_pull_done(model_id: &str, path: &str) {
    cli_intro("clai models · pull", model_id);
    cli_section("Saved");
    cli_kv("Model file", path);
    println!();
}

pub fn print_models_rm(path: &str) {
    cli_intro("clai models · rm", "file removed");
    cli_kv("Deleted", path);
    println!();
}

pub fn print_models_registry_updated(cache_path: &str, registry_version: u32) {
    cli_intro("clai models · update-registry", "cache written");
    cli_kv("Cache file", cache_path);
    cli_kv("Registry format version", &registry_version.to_string());
    println!();
}

pub fn print_models_default_set(model_id: &str, config_path: &str) {
    cli_intro("clai models · default set", "configuration saved");
    cli_kv("Default model", model_id);
    cli_kv("Config file", config_path);
    println!();
}

pub fn print_init_done(config_path: &str, default_model_id: &str) {
    cli_intro("clai init", "configuration saved");
    cli_section("Next steps");
    cli_kv("Config file", config_path);
    cli_note(&format!(
        "Download the model: clai models pull {default_model_id}"
    ));
    println!();
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
                    cli_kv("Command", &line);
                } else if needs_shell {
                    cli_kv("Shell (needs_shell)", &line);
                } else {
                    cli_kv("Run", &line);
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
            PreRunLine::WorkingDir(p) => cli_kv("Cwd", &p),
            PreRunLine::Intent(s) => cli_kv("Why", &s),
            PreRunLine::Confidence(c) => cli_kv("Confidence", &c),
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
    cli_intro(
        &format!("clai · interactive · {source}"),
        &format!("session mode: {mode}"),
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
    cli_kv("Model", ml);
    println!();
}

/// Unstyled prompt line (so print before readline; cursor after `clai> `).
pub fn print_clai_prompt() {
    if out_style() {
        print!("{}", Style::new().bold().white().apply_to("clai> "));
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
    cli_section("Built-ins");
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
    cli_section("Execution mode (this session)");
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

fn pretty_os_id(os: &str) -> String {
    match os {
        "macos" => "macOS".into(),
        "linux" => "Linux".into(),
        "windows" => "Windows".into(),
        _ => os.to_string(),
    }
}

fn env_option_line(name: &str, value: Option<&str>, when_unset_hint: &str) {
    match value {
        Some(v) if !v.is_empty() => cli_kv(name, v),
        _ => cli_kv(name, when_unset_hint),
    }
}

/// `clai doctor`: grouped, readable diagnostics (TTY-aware styling).
#[allow(clippy::too_many_arguments)]
pub fn print_doctor_report(
    host: &HostContext,
    registry_version: u32,
    config_version: u32,
    dry_run_default: bool,
    effective_interactive: InteractiveExecutionMode,
    interactive_from_config: Option<InteractiveExecutionMode>,
    execution_mode: ExecutionMode,
    docker_image: Option<&str>,
    data_dir: &str,
    model_path: Result<String, String>,
    clai_n_gpu_layers: Option<&str>,
    clai_json_schema_grammar: Option<&str>,
    llama_feature: bool,
) {
    cli_intro("clai doctor", "environment this install will use");

    cli_section("Host");
    let system_line = format!(
        "{} — {}",
        pretty_os_id(&host.os),
        host.os_description.trim()
    );
    cli_kv("System", &system_line);
    cli_kv("Architecture", &host.arch);
    let shell_line = match host.shell_executable_hint.as_deref() {
        Some(path) => format!("{} ({})", host.shell_family.user_label(), path),
        None => host.shell_family.user_label().to_string(),
    };
    cli_kv("Shell", &shell_line);
    let tty = if host.is_tty {
        "yes — stdout is a terminal"
    } else {
        "no — stdout is piped or redirected"
    };
    cli_kv("TTY (stdout)", tty);
    cli_kv("Working directory", &host.cwd);
    cli_kv("Path separator", &host.path_separator.to_string());

    cli_section("Model catalog");
    cli_kv("Registry format version", &registry_version.to_string());

    cli_section("Configuration file");
    cli_kv("Config schema version", &config_version.to_string());
    cli_kv(
        "Legacy policy.dry_run_default",
        if dry_run_default {
            "true (maps to interactive dry-run when [interactive].execution is unset)"
        } else {
            "false (maps to confirm when [interactive].execution is unset)"
        },
    );
    let cfg_exec_note = match interactive_from_config {
        Some(m) => format!("set to `{}` in config / env", m.as_str()),
        None => "not set (see legacy line above for fallback)".to_string(),
    };
    cli_kv("[interactive].execution", &cfg_exec_note);
    cli_kv(
        "Effective interactive mode",
        &format!(
            "{} (config + environment only; `--yes` / `--interactive-mode` are not applied here)",
            effective_interactive.as_str()
        ),
    );

    cli_section("Command execution");
    match execution_mode {
        ExecutionMode::Direct => {
            cli_kv("Mode", "direct — run on this machine");
            cli_kv("Docker image", "(not used)");
        }
        ExecutionMode::Docker => {
            cli_kv("Mode", "docker — run inside a container");
            let img = docker_image
                .filter(|s| !s.is_empty())
                .unwrap_or("alpine:latest (default when unset)");
            cli_kv("Docker image", img);
        }
        ExecutionMode::Bwrap => {
            cli_kv("Mode", "bwrap — Bubblewrap sandbox on this machine");
            cli_kv("Docker image", "(not used)");
        }
    }

    cli_section("Paths");
    cli_kv("Data directory", data_dir);
    match model_path {
        Ok(ref p) => cli_kv("Model file", p),
        Err(ref e) => {
            cli_kv("Model file", "(not resolved)");
            cli_note(e);
        }
    }

    cli_section("Environment overrides");
    env_option_line(
        "CLAI_N_GPU_LAYERS",
        clai_n_gpu_layers,
        "not set (backend default)",
    );
    match clai_json_schema_grammar {
        Some(v) if !v.is_empty() => {
            cli_kv("CLAI_JSON_SCHEMA_GRAMMAR", v);
            cli_note("GBNF JSON schema sampling is enabled; some llama.cpp builds abort if this is on.");
        }
        _ => {
            cli_kv("CLAI_JSON_SCHEMA_GRAMMAR", "not set (off — recommended)");
            cli_note("Turn on only if you need schema-constrained decoding and your build supports it.");
        }
    }

    cli_section("Build");
    if llama_feature {
        cli_kv("Local inference", "enabled (llama / embedded llama.cpp)");
    } else {
        cli_kv(
            "Local inference",
            "disabled (built without `llama`; cloud or stubs only)",
        );
    }

    println!();
}
