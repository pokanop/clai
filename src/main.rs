#![allow(clippy::result_large_err, clippy::field_reassign_with_default)]

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use clai::app_update;
use clai::ask_exit::{CLAI_ASK_DRY_RUN_EXIT, CLAI_ASK_USER_DECLINED_EXIT};
use clai::cli_output::{
    cli_intro, cli_note, cli_section, eprint_captured_stream_encoding_note, print_doctor_report,
    print_init_done, print_models_default_set, print_models_list, print_models_ollama,
    print_models_pull_done, print_models_registry_updated, print_models_rm, print_models_search,
    print_pre_run, print_proposal_json, print_run_hint, print_verbose_run_report, ModelCatalogRow,
};
use clai::cloud;
use clai::config::{
    self, default_config_path, default_data_dir, default_models_dir, default_registry_cache_path,
    installed_model_path, resolve_registry_cache_path_for_read, AppConfig, ExecutionConfig,
    ExecutionMode,
};
use clai::executor;
use clai::host_context::HostContext;
use clai::interactive_mode::{
    resolve_effective_interactive_execution_mode, InteractiveExecutionMode,
};
use clai::migrate;
use clai::ollama;
use clai::policy::PolicyEngine;
use clai::registry::{self, ModelRegistry};
use clai::schema::CommandProposal;
use clai::stream_strategy::{
    current_user_terminal_context, select_stream_strategy, OutputIntent, StreamStrategy,
};
use clai::tty::{eprintln_labeled, println_labeled, Severity};
use clai::Result;

/// Natural-language → local command (embedded GGUF optional).
///
/// On a **TTY** (stdin and stdout), running `clai` with no subcommand starts the **interactive session**
/// (same as `clai interactive`). For automation, use `clai ask '…'` or pass a subcommand explicitly.
/// Local sessions: optional eager GGUF load via `[interactive].local_warmup` or `CLAI_INTERACTIVE__LOCAL_WARMUP`
/// (default off; see README).
#[derive(Parser, Debug)]
#[command(name = "clai", version, about, subcommand_required = false)]
struct Cli {
    #[arg(long, global = true, help = "Path to config.toml")]
    config: Option<PathBuf>,

    #[arg(long, global = true, help = "Override model GGUF path")]
    model: Option<PathBuf>,

    #[arg(
        long = "interactive-mode",
        global = true,
        value_enum,
        env = "CLAI_INTERACTIVE__EXECUTION",
        help = "Default interactive session execution: dry-run | confirm | auto. Precedence: --yes > this flag > CLAI_INTERACTIVE__EXECUTION / [interactive] in config > legacy policy.dry_run_default mapping (see README)"
    )]
    interactive_mode: Option<InteractiveExecutionMode>,

    #[arg(
        long,
        global = true,
        help = "Force automatic interactive execution (auto) and auto-confirm policy prompts. Applies to bare `clai` / `clai interactive` and to `clai ask` when placed before the subcommand (e.g. `clai --yes ask ...`)"
    )]
    yes: bool,

    #[arg(
        long,
        global = true,
        help = "Use cloud completion if cloud.enabled (bare session and `clai --cloud ask`)"
    )]
    cloud: bool,

    #[arg(
        long,
        short = 'v',
        global = true,
        env = "CLAI_ASK_VERBOSE",
        help = "Verbose machine-oriented output for `clai ask` and the default interactive session"
    )]
    verbose: bool,

    #[arg(
        long = "force-capture",
        global = true,
        env = "CLAI_ASK_FORCE_CAPTURE",
        help = "Force piped capture in direct mode (`ask` + interactive session)"
    )]
    force_capture: bool,

    #[arg(
        long = "no-preview",
        global = true,
        env = "CLAI_ASK_NO_PREVIEW",
        help = "Omit one-line Run: preview (`ask` + interactive session)"
    )]
    no_preview: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Interactive natural-language session (same as bare `clai` on a TTY)
    Interactive,
    /// First-run wizard
    Init,
    /// Show host, model, and backend diagnostics
    Doctor,
    /// Ask in natural language; proposes and optionally runs a command.
    ///
    /// Global flags placed **before** `ask` also apply (e.g. `clai --yes ask …`, `clai -v ask …`,
    /// `clai --cloud ask …`). See top-level `clai --help` for `--interactive-mode`, `--yes`, `--cloud`,
    /// `--verbose`, `--force-capture`, and `--no-preview`.
    Ask {
        #[arg(trailing_var_arg = true, required = true)]
        words: Vec<String>,
        #[arg(
            long,
            help = "Only print the proposed argv as JSON, then exit (does not run the command; same proposal shape as a normal run). Combine with --verbose to force the pre-exec pretty-printed proposal even in future minimal-default modes"
        )]
        print_only: bool,
        #[arg(
            long,
            short = 'v',
            env = "CLAI_ASK_VERBOSE",
            help = "Opt in to full pretty-printed proposal before run; forces captured streams (use for audits and CI). Set ask_verbose in config.toml or CLAI_ASK_VERBOSE=1"
        )]
        verbose: bool,
        #[arg(
            long = "force-capture",
            env = "CLAI_ASK_FORCE_CAPTURE",
            help = "On execution.mode = direct, use piped capture even when stdin/stdout/stderr are TTYs (applies size limits; policy unchanged). Or ask_force_capture in config.toml / CLAI_ASK_FORCE_CAPTURE=1"
        )]
        force_capture: bool,
        #[arg(
            long = "no-preview",
            env = "CLAI_ASK_NO_PREVIEW",
            help = "Omit the one-line pre-run hint (Run: … or non-direct context) in default human mode. Or ask_no_preview in config / CLAI_ASK_NO_PREVIEW=1"
        )]
        no_preview: bool,
        #[arg(
            long,
            short = 'y',
            help = "Auto-confirm policy prompts (use carefully)"
        )]
        yes: bool,
        #[arg(long, help = "Use cloud OpenAI-compatible API from config")]
        cloud: bool,
    },
    /// List, search, pull, and manage GGUF models (catalog + optional Ollama discovery).
    #[command(subcommand)]
    Models(ModelsCmd),
    #[command(name = "self", subcommand)]
    Me(MeCmd),
    #[command(subcommand)]
    Migrate(MigrateCmd),
}

#[derive(Subcommand, Debug)]
enum ModelsCmd {
    List {
        #[arg(
            short,
            long,
            help = "Show Hugging Face repo and GGUF filename for each entry"
        )]
        verbose: bool,
    },
    Search {
        query: String,
    },
    Pull {
        id: String,
        #[arg(long, help = "Verify sha256 when registry provides it")]
        verify: bool,
    },
    #[command(name = "default")]
    Default {
        #[command(subcommand)]
        action: DefaultModelCmd,
    },
    Rm {
        id: String,
    },
    #[command(name = "update-registry")]
    UpdateRegistry {
        #[arg(long, help = "URL to registry.json")]
        url: Option<String>,
    },
    /// List tags from a local `ollama serve` (discovery only; clai still loads GGUF itself).
    Ollama {
        #[arg(
            long,
            env = "CLAI_OLLAMA_HOST",
            help = "Ollama HTTP API base, e.g. http://127.0.0.1:11434"
        )]
        host: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum DefaultModelCmd {
    Set { id: String },
}

#[derive(Subcommand, Debug)]
enum MeCmd {
    /// Update this binary from GitHub Releases
    Update {
        #[arg(long, help = "Only show latest release tag")]
        check: bool,
        #[arg(long, env = "CLAI_UPDATE_REPO_OWNER", default_value = "pokanop")]
        owner: String,
        #[arg(long, env = "CLAI_UPDATE_REPO_NAME", default_value = "clai")]
        repo: String,
        /// Target triple to match in the release asset file name (default: compile-time TARGET)
        #[arg(long, env = "CLAI_UPDATE_TARGET")]
        target: Option<String>,
        /// Path inside the release archive; supports {{ bin }}, {{ target }}, {{ version }}
        #[arg(long, env = "CLAI_UPDATE_BIN_PATH_IN_ARCHIVE")]
        bin_path_in_archive: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum MigrateCmd {
    /// Show migration plan
    #[command(name = "dry-run")]
    DryRun,
    /// Apply migrations
    Apply,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln_labeled("error", &e.to_string(), Severity::Error);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => cmd_default_entry(cli),
        Some(Commands::Interactive) => cmd_default_entry(cli),
        Some(Commands::Init) => cmd_init(cli.config),
        Some(Commands::Doctor) => cmd_doctor(cli.config, cli.model),
        Some(Commands::Ask {
            words,
            print_only,
            verbose,
            force_capture,
            no_preview,
            yes,
            cloud,
        }) => cmd_ask(
            cli.config,
            cli.model,
            words.join(" "),
            print_only,
            verbose || cli.verbose,
            force_capture || cli.force_capture,
            no_preview || cli.no_preview,
            yes || cli.yes,
            cloud || cli.cloud,
        ),
        Some(Commands::Models(m)) => cmd_models(cli.config, m),
        Some(Commands::Me(m)) => cmd_me(m),
        Some(Commands::Migrate(m)) => cmd_migrate(cli.config, m),
    }
}

/// Exit code when stdin or stdout is not a TTY and the user invokes bare `clai` / `clai interactive`.
const NON_TTY_DEFAULT_EXIT: i32 = 2;

fn cmd_default_entry(cli: Cli) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln_labeled(
            "clai",
            "interactive mode needs a terminal on stdin and stdout. For scripts, use: clai ask '…'",
            Severity::Warn,
        );
        eprintln_labeled("hint", "Run `clai --help` for usage.", Severity::Info);
        std::process::exit(NON_TTY_DEFAULT_EXIT);
    }
    cmd_interactive(cli)
}

fn merged_registry(config_path: Option<PathBuf>) -> Result<ModelRegistry> {
    let cache = resolve_registry_cache_path_for_read();
    let cfg = config::load_config_raw(config_path)?;
    ModelRegistry::load_merged_with_config(&cache, &cfg.models.extra)
}

fn cmd_interactive(cli: Cli) -> Result<()> {
    let cfg = load_cfg(cli.config.clone())?;
    let reg = merged_registry(cli.config.clone())?;
    let host = HostContext::gather(
        cfg.preferred_shell.as_deref(),
        cfg.execution_profile.as_deref(),
    );
    let system = build_system_prompt(&host);
    clai::session::run_interactive_session(
        cfg,
        cli.model,
        &reg,
        resolve_model_path,
        &host,
        &system,
        cli.interactive_mode,
        cli.yes,
        cli.cloud,
        cli.verbose,
        cli.force_capture,
        cli.no_preview,
    )
}

fn load_cfg(path: Option<PathBuf>) -> Result<AppConfig> {
    AppConfig::load(path.clone()).map_err(|e| {
        if matches!(&e, clai::AppError::ConfigNeedsMigration { .. }) {
            clai::AppError::Msg("config requires migration — run: clai migrate apply".into())
        } else {
            e
        }
    })
}

fn profile_rank(profile: &str) -> u8 {
    match profile {
        "fast" => 0,
        "balanced" => 1,
        "capable" => 2,
        _ => 3,
    }
}

fn cmd_init(config_path: Option<PathBuf>) -> Result<()> {
    let reg = merged_registry(config_path.clone())?;
    let mut models = reg.models.clone();
    models.sort_by(|a, b| {
        profile_rank(&a.profile)
            .cmp(&profile_rank(&b.profile))
            .then_with(|| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            })
    });
    let picked = inquire::Select::new(
        "Default local GGUF model (catalog + optional [[models.extra]] in config)",
        models,
    )
    .with_formatter(&|opt| {
        let r = opt.value;
        let ram = r
            .ram_hint_gb
            .map(|g| format!("~{g} GB RAM"))
            .unwrap_or_else(|| "RAM ?".into());
        format!(
            "{:<40}  {:<10}  {}",
            r.display_name.as_str(),
            r.profile.as_str(),
            ram
        )
    })
    .prompt()
    .map_err(|e| clai::AppError::Msg(e.to_string()))?;
    let id = picked.id;

    let strict = inquire::Confirm::new("Enable dry-run by default for new commands?")
        .with_default(true)
        .prompt()
        .map_err(|e| clai::AppError::Msg(e.to_string()))?;

    let c = AppConfig {
        default_model_id: Some(id.clone()),
        policy: clai::config::PolicyConfig {
            dry_run_default: strict,
            ..Default::default()
        },
        ..Default::default()
    };
    let written = config_path
        .clone()
        .unwrap_or_else(default_config_path)
        .display()
        .to_string();
    c.save(config_path)?;
    print_init_done(&written, id.as_str());
    Ok(())
}

fn cmd_doctor(config_path: Option<PathBuf>, model_override: Option<PathBuf>) -> Result<()> {
    let reg = merged_registry(config_path.clone())?;
    let cfg = config::load_config_raw(config_path.clone()).unwrap_or_default();
    let host = HostContext::gather(
        cfg.preferred_shell.as_deref(),
        cfg.execution_profile.as_deref(),
    );
    let effective_interactive = resolve_effective_interactive_execution_mode(
        cfg.interactive.execution,
        None,
        false,
        cfg.policy.dry_run_default,
    );
    let model_path = resolve_model_path(&cfg, model_override, &reg)
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string());

    print_doctor_report(
        &host,
        reg.registry_version,
        cfg.config_version,
        cfg.policy.dry_run_default,
        effective_interactive,
        cfg.interactive.execution,
        cfg.execution.mode,
        cfg.execution.docker_image.as_deref(),
        &default_data_dir().display().to_string(),
        model_path,
        std::env::var("CLAI_N_GPU_LAYERS").ok().as_deref(),
        std::env::var("CLAI_JSON_SCHEMA_GRAMMAR").ok().as_deref(),
        cfg!(feature = "llama-embed"),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_ask(
    config_path: Option<PathBuf>,
    model_override: Option<PathBuf>,
    prompt: String,
    print_only: bool,
    verbose: bool,
    force_capture: bool,
    no_preview: bool,
    yes: bool,
    use_cloud: bool,
) -> Result<()> {
    let cfg = load_cfg(config_path.clone())?;
    let reg = merged_registry(config_path.clone())?;
    let host = HostContext::gather(
        cfg.preferred_shell.as_deref(),
        cfg.execution_profile.as_deref(),
    );
    let system = build_system_prompt(&host);
    let user = format!("User request: {}\nReply with ONLY the JSON object.", prompt);
    let no_stream = std::env::var("CLAI_NO_STREAM")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));

    let raw = if use_cloud && cfg.cloud.enabled {
        if !no_stream {
            clai::cli_output::eprint_cloud_request_prelude();
        }
        let base = cfg
            .cloud
            .base_url
            .as_deref()
            .ok_or_else(|| clai::AppError::Msg("cloud.base_url missing".into()))?;
        let model = cfg
            .cloud
            .model
            .as_deref()
            .ok_or_else(|| clai::AppError::Msg("cloud.model missing".into()))?;
        let key = cfg
            .cloud
            .api_key_env
            .as_deref()
            .and_then(|e| std::env::var(e).ok());
        cloud::complete_cloud(
            base,
            key.as_deref(),
            model,
            &system,
            &user,
            cfg.cloud.structured_outputs,
        )?
    } else {
        let path = resolve_model_path(&cfg, model_override, &reg)?;
        #[cfg(feature = "llama-embed")]
        {
            let phase_verbose = verbose || cfg.ask_verbose;
            if !no_stream {
                clai::cli_output::eprint_model_stream_prelude();
            }
            let stream = !no_stream;
            let r = clai::engine::complete_local_with(
                &path,
                &system,
                &user,
                clai::engine::max_new_tokens_local(),
                phase_verbose,
                |piece: &str| {
                    if stream {
                        clai::cli_output::eprint_model_stream_piece(piece);
                    }
                },
            );
            if stream {
                clai::cli_output::eprint_model_stream_end();
            }
            r.map_err(clai::AppError::Msg)?
        }
        #[cfg(not(feature = "llama-embed"))]
        {
            clai::engine::complete_local_best_effort(
                &path,
                &system,
                &user,
                clai::engine::max_new_tokens_local(),
            )?
        }
    };

    let proposal = CommandProposal::parse_from_model_text(&raw)?;
    if print_only {
        print_proposal_json(&proposal)?;
        cli_note("(print-only; not executed)");
        println!();
        return Ok(());
    }

    let verbose_ask = verbose || cfg.ask_verbose;
    let force_capture = force_capture || cfg.ask_force_capture;
    let no_preview = no_preview || cfg.ask_no_preview;
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
    if verbose_ask && io::stdout().is_terminal() {
        print_pre_run(&proposal, &decision);
    }
    if decision.blocked {
        return Err(clai::AppError::Msg(
            decision
                .reason
                .unwrap_or_else(|| "blocked by policy".into()),
        ));
    }

    if decision.requires_confirmation && !yes {
        let ok = inquire::Confirm::new("This command is sensitive or destructive. Run it?")
            .with_default(false)
            .prompt()
            .map_err(|e| clai::AppError::Msg(e.to_string()))?;
        if !ok {
            println_labeled("clai", "Aborted (confirmation declined).", Severity::Warn);
            std::process::exit(CLAI_ASK_USER_DECLINED_EXIT);
        }
    }

    if cfg.policy.dry_run_default && !yes {
        println_labeled(
            "clai",
            "Dry-run: command not executed (policy.dry_run_default).",
            Severity::Info,
        );
        std::process::exit(CLAI_ASK_DRY_RUN_EXIT);
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
            print_run_hint(&format!("Run: {}", ask_command_line_preview(&proposal)));
        }
    }
    let out = executor::run_proposal(
        &proposal,
        Duration::from_secs(120),
        256 * 1024,
        &cfg.execution,
        stream,
    )?;
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
                    if let Some(line) = non_direct_context_one_line(&proposal, &cfg.execution)? {
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
    std::process::exit(out.clai_ask_process_exit);
}

/// One-line argv preview (program + args only) for default human `ask`; omits `reason`/`cwd` and
/// other model fields so we do not add policy-bypass or secret guidance (FR-5).
fn ask_command_line_preview(p: &CommandProposal) -> String {
    let mut s = shell_escape_for_display(&p.program);
    for a in &p.args {
        s.push(' ');
        s.push_str(&shell_escape_for_display(a));
    }
    s
}

/// Display-only quoting for a single argv token.
fn shell_escape_for_display(t: &str) -> String {
    if t.is_empty() {
        return "''".to_string();
    }
    if t.chars()
        .any(|c| c.is_whitespace() || matches!(c, '\\' | '\'' | '"'))
    {
        format!("'{}'", t.replace('\'', "'\"'\"'"))
    } else {
        t.to_string()
    }
}

fn effective_proposal_cwd(proposal: &CommandProposal) -> std::io::Result<PathBuf> {
    if let Some(c) = &proposal.cwd {
        Ok(PathBuf::from(c))
    } else {
        std::env::current_dir()
    }
}

/// Program, working directory, and `execution.mode` in one line for container/sandbox runs (FR-6).
/// Returns `None` when `mode` is `direct`.
fn non_direct_context_one_line(
    proposal: &CommandProposal,
    execution: &ExecutionConfig,
) -> std::io::Result<Option<String>> {
    let cwd = effective_proposal_cwd(proposal)?;
    let cmd = ask_command_line_preview(proposal);
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

/// Extra wrapper metadata for verbose `ask` (operators).
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
mod ask_preview_tests {
    use super::{ask_command_line_preview, shell_escape_for_display};
    use clai::schema::CommandProposal;

    fn proposal(program: &str, args: &[&str]) -> CommandProposal {
        CommandProposal {
            program: program.to_string(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        }
    }

    #[test]
    fn preview_program_and_args() {
        assert_eq!(ask_command_line_preview(&proposal("ls", &[])), "ls");
        assert_eq!(
            ask_command_line_preview(&proposal("ls", &["-la"])),
            "ls -la"
        );
    }

    #[test]
    fn preview_omits_model_metadata() {
        let p = CommandProposal {
            program: "true".to_string(),
            args: vec![],
            cwd: None,
            reason: Some("secret or bypass hint from model".to_string()),
            needs_shell: false,
            confidence: None,
        };
        let line = ask_command_line_preview(&p);
        assert!(!line.contains("reason"));
        assert!(!line.contains("bypass"));
    }

    #[test]
    fn escape_quotes_arg_with_space() {
        assert_eq!(
            ask_command_line_preview(&proposal("echo", &["a b"])),
            "echo 'a b'"
        );
    }

    #[test]
    fn escape_empty_token() {
        assert_eq!(shell_escape_for_display(""), "''");
    }
}

#[cfg(test)]
mod non_direct_context_tests {
    use clai::config::{ExecutionConfig, ExecutionMode};

    use super::non_direct_context_one_line;
    use clai::schema::CommandProposal;

    fn p() -> CommandProposal {
        CommandProposal {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "true".to_string()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        }
    }

    #[test]
    fn direct_mode_returns_none() {
        let e = ExecutionConfig {
            mode: ExecutionMode::Direct,
            ..Default::default()
        };
        assert!(non_direct_context_one_line(&p(), &e).unwrap().is_none());
    }

    #[test]
    fn docker_includes_image_and_cwd() {
        let e = ExecutionConfig {
            mode: ExecutionMode::Docker,
            docker_image: Some("myimg:tag".into()),
            ..Default::default()
        };
        let line = non_direct_context_one_line(&p(), &e)
            .unwrap()
            .expect("line");
        assert!(line.contains("profile=docker"));
        assert!(line.contains("image=myimg:tag"));
        assert!(line.contains("sh"));
        assert!(line.contains("cwd="));
    }

    #[test]
    fn bwrap_contains_profile() {
        let e = ExecutionConfig {
            mode: ExecutionMode::Bwrap,
            ..Default::default()
        };
        let line = non_direct_context_one_line(&p(), &e)
            .unwrap()
            .expect("line");
        assert!(line.contains("profile=bwrap"));
    }
}

fn cmd_models(config_path: Option<PathBuf>, m: ModelsCmd) -> Result<()> {
    let cache_write = default_registry_cache_path();
    match m {
        ModelsCmd::List { verbose } => {
            let reg = merged_registry(config_path.clone())?;
            let cfg = config::load_config_raw(config_path.clone()).unwrap_or_default();
            let rows: Vec<ModelCatalogRow> = reg
                .models
                .iter()
                .map(|m| {
                    let is_default = cfg
                        .default_model_id
                        .as_deref()
                        .map(|d| d == m.id.as_str())
                        .unwrap_or(false);
                    let location = installed_model_path(&m.filename)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(not downloaded)".into());
                    ModelCatalogRow {
                        id: m.id.clone(),
                        display_name: m.display_name.clone(),
                        profile: m.profile.clone(),
                        location,
                        is_default,
                        hf_repo: m.hf_repo.clone(),
                        filename: m.filename.clone(),
                        ram_hint_gb: m.ram_hint_gb,
                    }
                })
                .collect();
            print_models_list(&rows, verbose);
        }
        ModelsCmd::Search { query } => {
            let reg = merged_registry(config_path.clone())?;
            let hits: Vec<(&str, &str)> = reg
                .search(&query)
                .into_iter()
                .map(|m| (m.id.as_str(), m.display_name.as_str()))
                .collect();
            print_models_search(&query, &hits);
        }
        ModelsCmd::Pull { id, verify } => {
            let reg = merged_registry(config_path.clone())?;
            let m = reg
                .find(&id)
                .ok_or_else(|| clai::AppError::Msg(format!("unknown model {}", id)))?;
            let p = registry::pull_model(m, &default_models_dir(), verify)?;
            print_models_pull_done(&id, &p.display().to_string());
        }
        ModelsCmd::Default { action } => {
            let mut cfg = load_cfg(config_path.clone()).unwrap_or_default();
            match action {
                DefaultModelCmd::Set { id } => {
                    let reg = merged_registry(config_path.clone())?;
                    reg.find(&id).ok_or_else(|| {
                        clai::AppError::Msg(format!(
                            "unknown model id `{id}` — run `clai models list` or add [[models.extra]]"
                        ))
                    })?;
                    cfg.default_model_id = Some(id.clone());
                    cfg.save(config_path.clone())?;
                    let written = config_path
                        .clone()
                        .unwrap_or_else(default_config_path)
                        .display()
                        .to_string();
                    print_models_default_set(&id, &written);
                }
            }
        }
        ModelsCmd::Rm { id } => {
            let reg = merged_registry(config_path.clone())?;
            let m = reg
                .find(&id)
                .ok_or_else(|| clai::AppError::Msg(format!("unknown model {}", id)))?;
            let p = installed_model_path(&m.filename)
                .ok_or_else(|| clai::AppError::Msg(format!("model file not present: {}", id)))?;
            let disp = p.display().to_string();
            std::fs::remove_file(&p)?;
            print_models_rm(&disp);
        }
        ModelsCmd::UpdateRegistry { url } => {
            let u = url.unwrap_or_else(|| {
                std::env::var("CLAI_REGISTRY_URL").unwrap_or_else(|_| {
                    "https://raw.githubusercontent.com/pokanop/clai/main/assets/registry.json"
                        .into()
                })
            });
            let body = ureq::get(&u)
                .call()
                .map_err(|e| clai::AppError::Msg(format!("fetch registry: {}", e)))?
                .into_string()
                .map_err(|e| clai::AppError::Msg(e.to_string()))?;
            let reg: ModelRegistry = serde_json::from_str(&body)?;
            registry::write_registry_cache(&cache_write, &reg)?;
            print_models_registry_updated(&cache_write.display().to_string(), reg.registry_version);
        }
        ModelsCmd::Ollama { host } => {
            let base = host.unwrap_or_else(|| "http://127.0.0.1:11434".into());
            let rows = ollama::list_local_tags(&base)?;
            print_models_ollama(&base, &rows);
        }
    }
    Ok(())
}

fn cmd_me(m: MeCmd) -> Result<()> {
    match m {
        MeCmd::Update {
            check,
            owner,
            repo,
            target,
            bin_path_in_archive,
        } => {
            app_update::self_update(
                &owner,
                &repo,
                "clai",
                env!("CARGO_PKG_VERSION"),
                check,
                target.as_deref(),
                bin_path_in_archive.as_deref(),
            )?;
        }
    }
    Ok(())
}

fn cmd_migrate(config_path: Option<PathBuf>, m: MigrateCmd) -> Result<()> {
    match m {
        MigrateCmd::DryRun => {
            let plan = migrate::dry_run(config_path)?;
            cli_intro("clai migrate · dry-run", "no changes written");
            cli_section("Plan");
            println!("{plan}");
            println!();
        }
        MigrateCmd::Apply => {
            migrate::apply(config_path)?;
            cli_intro("clai migrate · apply", "complete");
            cli_note("Configuration updated to the latest schema version.");
            println!();
        }
    }
    Ok(())
}

fn resolve_model_path(
    cfg: &AppConfig,
    override_path: Option<PathBuf>,
    reg: &ModelRegistry,
) -> Result<PathBuf> {
    if let Some(p) = override_path.or_else(|| cfg.model_path.clone()) {
        return Ok(p);
    }
    let id = cfg
        .default_model_id
        .as_deref()
        .ok_or_else(|| clai::AppError::Msg("no default model; run clai init or --model".into()))?;
    registry::default_model_path_for(id, reg)
}

fn build_system_prompt(host: &HostContext) -> String {
    format!(
        "You output exactly one JSON object for running a CLI command. Schema:\n{}\n\
         Host: os={} arch={} cwd={} shell_family={:?} path_sep={}\n\
         Rules: use program + args (argv). No markdown. No prose outside JSON.\n\
         Always populate \"reason\" with a concise explanation of WHY this argv fits the user request \
         (what problem it solves / why it is appropriate). If you refuse, use program \"echo\" and args [\"refused\"] and a reason field.",
        CommandProposal::schema_json(),
        host.os,
        host.arch,
        host.cwd,
        host.shell_family,
        host.path_separator
    )
}
