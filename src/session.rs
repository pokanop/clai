//! Interactive default-session loop (FR-3, FR-10, FR-11, FR-16, NFR-4).

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::Duration;

use crate::cli_output::{
    eprint_captured_stream_encoding_note, eprint_cloud_request_prelude, print_clai_prompt,
    print_dry_run_skip_note, print_pre_run, print_proposal_json, print_run_hint,
    print_session_help_styled, print_session_start, print_verbose_run_report,
};
#[cfg(feature = "llama")]
use crate::cli_output::{
    eprint_model_stream_end, eprint_model_stream_piece, eprint_model_stream_prelude,
};
use crate::cloud;
use crate::config::{AppConfig, ExecutionConfig, ExecutionMode};
#[cfg(feature = "llama")]
use crate::engine::max_new_tokens_local;
use crate::executor;
use crate::host_context::HostContext;
use crate::interactive_mode::{
    needs_dry_run_execute_prompt, needs_interactive_run_prompt,
    resolve_effective_interactive_execution_mode, InteractiveExecutionMode,
};
use crate::policy::PolicyEngine;
use crate::registry::ModelRegistry;
use crate::schema::CommandProposal;
use crate::stream_strategy::{
    current_user_terminal_context, select_stream_strategy, OutputIntent, StreamStrategy,
};
use crate::tty::{eprintln_labeled, println_labeled, Severity};
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionBuiltin {
    Exit,
    Help,
    Reload,
}

#[must_use]
pub fn classify_builtin_line(trimmed: &str) -> Option<SessionBuiltin> {
    let t = trimmed.trim();
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("exit") || t.eq_ignore_ascii_case("quit") {
        return Some(SessionBuiltin::Exit);
    }
    if t.eq_ignore_ascii_case("help") || t.eq_ignore_ascii_case("?") {
        return Some(SessionBuiltin::Help);
    }
    if t.eq_ignore_ascii_case("reload") {
        return Some(SessionBuiltin::Reload);
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub fn run_interactive_session(
    cfg: AppConfig,
    model_override: Option<PathBuf>,
    reg: &ModelRegistry,
    resolve_model_path: impl Fn(&AppConfig, Option<PathBuf>, &ModelRegistry) -> Result<PathBuf>,
    _host: &HostContext,
    system_prompt: &str,
    cli_interactive_mode: Option<InteractiveExecutionMode>,
    global_yes: bool,
    use_cloud: bool,
    verbose_cli: bool,
    force_capture_cli: bool,
    no_preview_cli: bool,
) -> Result<()> {
    let effective = resolve_effective_interactive_execution_mode(
        cfg.interactive.execution,
        cli_interactive_mode,
        global_yes,
        cfg.policy.dry_run_default,
    );

    let source = if use_cloud && cfg.cloud.enabled {
        "cloud"
    } else {
        "local"
    };
    let model_line = if use_cloud && cfg.cloud.enabled {
        let mid = cfg
            .cloud
            .model
            .as_deref()
            .unwrap_or("(cloud.model not set)");
        format!("model (cloud): {mid}")
    } else {
        match resolve_model_path(&cfg, model_override.clone(), reg) {
            Ok(p) => format!("model (local): {}", p.display()),
            Err(e) => format!("model (local): (unresolved — {e})"),
        }
    };
    print_session_start(effective, source, &model_line);

    #[cfg(feature = "llama")]
    let mut local_session: Option<crate::engine::LocalLlamaSession> = None;

    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        print_clai_prompt();
        let n = match stdin.read_line(&mut line) {
            Ok(n) => n,
            Err(e) => {
                eprintln_labeled("error", &format!("read stdin: {e}"), Severity::Error);
                continue;
            }
        };
        if n == 0 {
            println_labeled("clai", "EOF — goodbye.", Severity::Info);
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(b) = classify_builtin_line(trimmed) {
            match b {
                SessionBuiltin::Exit => {
                    println_labeled("clai", "Goodbye.", Severity::Info);
                    return Ok(());
                }
                SessionBuiltin::Help => {
                    print_session_help_styled(effective);
                    continue;
                }
                SessionBuiltin::Reload => {
                    #[cfg(feature = "llama")]
                    {
                        if use_cloud && cfg.cloud.enabled {
                            eprintln_labeled(
                                "warn",
                                "`reload` applies to local GGUF sessions only.",
                                Severity::Warn,
                            );
                            continue;
                        }
                        let path = match resolve_model_path(&cfg, model_override.clone(), reg) {
                            Ok(p) => p,
                            Err(e) => {
                                eprintln_labeled(
                                    "error",
                                    &format!("cannot resolve model path: {e:?}"),
                                    Severity::Error,
                                );
                                continue;
                            }
                        };
                        match local_session.as_mut() {
                            Some(s) => {
                                if let Err(e) = s.reload() {
                                    eprintln_labeled(
                                        "error",
                                        &format!("reload failed: {e}"),
                                        Severity::Error,
                                    );
                                } else {
                                    println_labeled(
                                        "ok",
                                        &format!("reloaded model from {}", path.display()),
                                        Severity::Ok,
                                    );
                                }
                            }
                            None => match crate::engine::LocalLlamaSession::open(&path) {
                                Ok(s) => {
                                    local_session = Some(s);
                                    println_labeled(
                                        "ok",
                                        &format!("loaded model from {}", path.display()),
                                        Severity::Ok,
                                    );
                                }
                                Err(e) => {
                                    eprintln_labeled(
                                        "error",
                                        &format!("load failed: {e}"),
                                        Severity::Error,
                                    );
                                }
                            },
                        }
                    }
                    #[cfg(not(feature = "llama"))]
                    {
                        eprintln_labeled(
                            "warn",
                            "reload requires a build with the `llama` feature.",
                            Severity::Warn,
                        );
                    }
                    continue;
                }
            }
        }

        let user = format!(
            "User request: {}\nReply with ONLY the JSON object.",
            trimmed
        );

        let no_stream = std::env::var("CLAI_NO_STREAM")
            .ok()
            .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
        let raw: String = if use_cloud && cfg.cloud.enabled {
            if !no_stream {
                eprint_cloud_request_prelude();
            }
            let base = cfg
                .cloud
                .base_url
                .as_deref()
                .ok_or_else(|| crate::AppError::Msg("cloud.base_url missing".into()))?;
            let model = cfg
                .cloud
                .model
                .as_deref()
                .ok_or_else(|| crate::AppError::Msg("cloud.model missing".into()))?;
            let key = cfg
                .cloud
                .api_key_env
                .as_deref()
                .and_then(|e| std::env::var(e).ok());
            match cloud::complete_cloud(
                base,
                key.as_deref(),
                model,
                system_prompt,
                &user,
                cfg.cloud.structured_outputs,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln_labeled(
                        "error",
                        &format!("cloud completion failed: {e:?}"),
                        Severity::Error,
                    );
                    continue;
                }
            }
        } else {
            #[cfg(feature = "llama")]
            {
                let path = match resolve_model_path(&cfg, model_override.clone(), reg) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln_labeled("error", &format!("model: {e:?}"), Severity::Error);
                        continue;
                    }
                };
                let stream = !no_stream;
                if stream {
                    eprint_model_stream_prelude();
                }
                let on_token = |piece: &str| {
                    if stream {
                        eprint_model_stream_piece(piece);
                    }
                };
                let out: Result<String> = match local_session.as_mut() {
                    Some(ls) => ls
                        .complete(system_prompt, &user, max_new_tokens_local(), on_token)
                        .map_err(crate::AppError::Msg),
                    None => match crate::engine::LocalLlamaSession::open(&path) {
                        Ok(mut ls) => {
                            let r = ls
                                .complete(system_prompt, &user, max_new_tokens_local(), on_token)
                                .map_err(crate::AppError::Msg);
                            local_session = Some(ls);
                            r
                        }
                        Err(e) => Err(crate::AppError::Msg(e)),
                    },
                };
                if stream {
                    eprint_model_stream_end();
                }
                match out {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln_labeled(
                            "error",
                            &format!("local completion failed: {e:?}"),
                            Severity::Error,
                        );
                        continue;
                    }
                }
            }
            #[cfg(not(feature = "llama"))]
            {
                eprintln_labeled(
                    "error",
                    "local inference unavailable (build without `llama`).",
                    Severity::Error,
                );
                continue;
            }
        };

        let proposal: CommandProposal = match CommandProposal::parse_from_model_text(&raw) {
            Ok(p) => p,
            Err(e) => {
                eprintln_labeled("error", &format!("parse proposal: {e:?}"), Severity::Error);
                continue;
            }
        };

        let verbose_ask = verbose_cli || cfg.ask_verbose;
        let force_capture = force_capture_cli || cfg.ask_force_capture;
        let no_preview = no_preview_cli || cfg.ask_no_preview;

        if verbose_ask {
            print_proposal_json(&proposal)?;
        }

        let jail = std::env::current_dir()?;
        let policy = PolicyEngine::new(
            jail,
            cfg.policy.strict_allowlist,
            cfg.policy.allowlist_bins.clone(),
        );
        let decision = policy.evaluate(&proposal);

        print_pre_run(&proposal, &decision);

        if decision.blocked {
            continue;
        }

        if needs_dry_run_execute_prompt(effective, global_yes) {
            let ok = match inquire::Confirm::new("Dry-run mode: execute this command?")
                .with_default(false)
                .prompt()
            {
                Ok(v) => v,
                Err(e) => {
                    eprintln_labeled("error", &format!("prompt failed: {e}"), Severity::Error);
                    continue;
                }
            };
            if !ok {
                print_dry_run_skip_note();
                continue;
            }
        }

        if decision.requires_confirmation && !global_yes {
            let ok =
                match inquire::Confirm::new("This command is sensitive or destructive. Run it?")
                    .with_default(false)
                    .prompt()
                {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln_labeled("error", &format!("prompt failed: {e}"), Severity::Error);
                        continue;
                    }
                };
            if !ok {
                println_labeled(
                    "clai",
                    "Skipped (policy confirmation declined).",
                    Severity::Warn,
                );
                continue;
            }
        }

        if needs_interactive_run_prompt(effective, global_yes) {
            let ok = match inquire::Confirm::new("Run proposed command?")
                .with_default(false)
                .prompt()
            {
                Ok(v) => v,
                Err(e) => {
                    eprintln_labeled("error", &format!("prompt failed: {e}"), Severity::Error);
                    continue;
                }
            };
            if !ok {
                println_labeled("clai", "Skipped (run declined).", Severity::Info);
                continue;
            }
        }

        let output_intent = if verbose_ask {
            OutputIntent::Verbose
        } else {
            OutputIntent::Human
        };
        let stream = select_stream_strategy(
            cfg.execution.mode,
            output_intent,
            current_user_terminal_context(),
            force_capture,
        );
        let is_non_direct = matches!(
            cfg.execution.mode,
            ExecutionMode::Docker | ExecutionMode::Bwrap
        );

        if !verbose_ask && !no_preview && io::stdout().is_terminal() {
            if let Some(line) = non_direct_context_one_line(&proposal, &cfg.execution)? {
                print_run_hint(&line);
            } else {
                print_run_hint(&format!(
                    "Run: {}",
                    crate::presentation::command_line_for_display(&proposal)
                ));
            }
        }

        let out = match executor::run_proposal(
            &proposal,
            Duration::from_secs(120),
            256 * 1024,
            &cfg.execution,
            stream,
        ) {
            Ok(o) => o,
            Err(e) => {
                eprintln_labeled("error", &format!("execute: {e:?}"), Severity::Error);
                continue;
            }
        };

        if verbose_ask {
            let ctx = non_direct_context_verbose(&proposal, &cfg.execution)?;
            print_verbose_run_report(
                ctx.as_deref(),
                &format!("{:?}", out.status),
                &out.stdout,
                &out.stderr,
            );
            if out.stdout.contains('\u{FFFD}') || out.stderr.contains('\u{FFFD}') {
                eprint_captured_stream_encoding_note();
            }
        } else {
            match stream {
                StreamStrategy::Inherit => {}
                StreamStrategy::Capture => {
                    if is_non_direct && !no_preview && !io::stdout().is_terminal() {
                        if let Some(line) = non_direct_context_one_line(&proposal, &cfg.execution)?
                        {
                            println!("{line}");
                        }
                    }
                    if !out.stdout.is_empty() {
                        print!("{}", out.stdout);
                    }
                    if !out.stderr.is_empty() {
                        eprint!("{}", out.stderr);
                    }
                }
            }
        }

        if out.timed_out || out.clai_ask_process_exit != 0 {
            println_labeled(
                "warn",
                &format!(
                    "run finished with clai exit mapping {} (clai continues; type another line or exit)",
                    out.clai_ask_process_exit
                ),
                Severity::Warn,
            );
        } else {
            println_labeled("ok", "command finished.", Severity::Ok);
        }
    }
}

fn effective_proposal_cwd(proposal: &CommandProposal) -> std::io::Result<PathBuf> {
    if let Some(c) = &proposal.cwd {
        Ok(PathBuf::from(c))
    } else {
        std::env::current_dir()
    }
}

fn non_direct_context_one_line(
    proposal: &CommandProposal,
    execution: &ExecutionConfig,
) -> std::io::Result<Option<String>> {
    let cwd = effective_proposal_cwd(proposal)?;
    let cmd = crate::presentation::command_line_for_display(proposal);
    match execution.mode {
        ExecutionMode::Direct => Ok(None),
        ExecutionMode::Docker => {
            let img = execution.docker_image.as_deref().unwrap_or("alpine:latest");
            Ok(Some(format!(
                "clai: profile=docker  image={img}  cwd={}  {cmd}",
                cwd.display()
            )))
        }
        ExecutionMode::Bwrap => Ok(Some(format!(
            "clai: profile=bwrap  cwd={}  {cmd}",
            cwd.display()
        ))),
    }
}

fn non_direct_context_verbose(
    proposal: &CommandProposal,
    execution: &ExecutionConfig,
) -> std::io::Result<Option<String>> {
    let Some(mut s) = non_direct_context_one_line(proposal, execution)? else {
        return Ok(None);
    };
    if execution.mode == ExecutionMode::Docker && !execution.docker_extra_args.is_empty() {
        s.push_str("\n  docker_extra_args: ");
        s.push_str(&format!("{:?}", execution.docker_extra_args));
    }
    if execution.mode == ExecutionMode::Bwrap && !execution.bwrap_extra_args.is_empty() {
        s.push_str("\n  bwrap_extra_args: ");
        s.push_str(&format!("{:?}", execution.bwrap_extra_args));
    }
    Ok(Some(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_exit_quit() {
        assert_eq!(
            classify_builtin_line("  EXIT  "),
            Some(SessionBuiltin::Exit)
        );
        assert_eq!(classify_builtin_line("quit"), Some(SessionBuiltin::Exit));
    }

    #[test]
    fn builtin_help() {
        assert_eq!(classify_builtin_line("help"), Some(SessionBuiltin::Help));
        assert_eq!(classify_builtin_line("?"), Some(SessionBuiltin::Help));
    }

    #[test]
    fn non_builtin_none() {
        assert_eq!(classify_builtin_line("ls -la"), None);
    }
}
