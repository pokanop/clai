#![allow(clippy::result_large_err, clippy::field_reassign_with_default)]

use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use clai::app_update;
use clai::cloud;
use clai::config::{
    self, default_config_path, default_data_dir, default_models_dir, default_registry_cache_path,
    installed_model_path, resolve_registry_cache_path_for_read, AppConfig,
};
use clai::engine;
use clai::executor;
use clai::host_context::HostContext;
use clai::migrate;
use clai::policy::PolicyEngine;
use clai::registry::{self, ModelRegistry};
use clai::schema::CommandProposal;
use clai::Result;

/// Natural-language → local command (embedded GGUF optional).
#[derive(Parser, Debug)]
#[command(name = "clai", version, about)]
struct Cli {
    #[arg(long, global = true, help = "Path to config.toml")]
    config: Option<PathBuf>,

    #[arg(long, global = true, help = "Override model GGUF path")]
    model: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// First-run wizard
    Init,
    /// Show host, model, and backend diagnostics
    Doctor,
    /// Ask in natural language; proposes and optionally runs a command
    Ask {
        #[arg(trailing_var_arg = true, required = true)]
        words: Vec<String>,
        #[arg(long, help = "Only print the proposed JSON argv")]
        print_only: bool,
        #[arg(
            long,
            short = 'y',
            help = "Auto-confirm policy prompts (use carefully)"
        )]
        yes: bool,
        #[arg(long, help = "Use cloud OpenAI-compatible API from config")]
        cloud: bool,
    },
    #[command(subcommand)]
    Models(ModelsCmd),
    #[command(name = "self", subcommand)]
    Me(MeCmd),
    #[command(subcommand)]
    Migrate(MigrateCmd),
}

#[derive(Subcommand, Debug)]
enum ModelsCmd {
    List,
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
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init => cmd_init(cli.config),
        Commands::Doctor => cmd_doctor(cli.config, cli.model),
        Commands::Ask {
            words,
            print_only,
            yes,
            cloud,
        } => cmd_ask(
            cli.config,
            cli.model,
            words.join(" "),
            print_only,
            yes,
            cloud,
        ),
        Commands::Models(m) => cmd_models(cli.config, m),
        Commands::Me(m) => cmd_me(m),
        Commands::Migrate(m) => cmd_migrate(cli.config, m),
    }
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

fn cmd_init(config_path: Option<PathBuf>) -> Result<()> {
    let profile = inquire::Select::new("Model profile", vec!["fast", "balanced", "capable"])
        .prompt()
        .map_err(|e| clai::AppError::Msg(e.to_string()))?;

    let id = match profile {
        "fast" => "fast-qwen25-coder-3b-q4",
        "balanced" => "balanced-qwen25-coder-7b-q4",
        "capable" => "capable-qwen25-coder-14b-q4",
        _ => "balanced-qwen25-coder-7b-q4",
    };

    let strict = inquire::Confirm::new("Enable dry-run by default for new commands?")
        .with_default(true)
        .prompt()
        .map_err(|e| clai::AppError::Msg(e.to_string()))?;

    let c = AppConfig {
        default_model_id: Some(id.into()),
        policy: clai::config::PolicyConfig {
            dry_run_default: strict,
            ..Default::default()
        },
        ..Default::default()
    };
    c.save(config_path)?;

    println!(
        "Wrote {}. Run: clai models pull {}",
        default_config_path().display(),
        id
    );
    Ok(())
}

fn cmd_doctor(config_path: Option<PathBuf>, model_override: Option<PathBuf>) -> Result<()> {
    let host = HostContext::gather(None, None);
    println!("Host:\n{}", host.to_prompt_json());

    let reg = registry::ModelRegistry::load_merged(&resolve_registry_cache_path_for_read())?;
    println!("registry_version: {}", reg.registry_version);

    let cfg = config::load_config_raw(config_path.clone()).unwrap_or_default();
    println!("config_version: {}", cfg.config_version);
    println!("dry_run_default: {}", cfg.policy.dry_run_default);
    println!(
        "execution.mode: {:?} docker_image: {:?}",
        cfg.execution.mode, cfg.execution.docker_image
    );

    println!("data_dir: {}", default_data_dir().display());
    match resolve_model_path(&cfg, model_override, &reg) {
        Ok(p) => println!("model_path: {}", p.display()),
        Err(e) => println!("model_path: (unset) — {e:?}"),
    }
    println!(
        "CLAI_N_GPU_LAYERS: {:?}",
        std::env::var("CLAI_N_GPU_LAYERS").ok()
    );
    println!(
        "CLAI_JSON_SCHEMA_GRAMMAR: {:?} (GBNF sampler; default off — llama.cpp may abort if on)",
        std::env::var("CLAI_JSON_SCHEMA_GRAMMAR").ok()
    );
    #[cfg(feature = "llama")]
    println!("build: llama (embedded llama.cpp enabled)");
    #[cfg(not(feature = "llama"))]
    println!("build: no llama (tests / minimal)");
    Ok(())
}

fn cmd_ask(
    config_path: Option<PathBuf>,
    model_override: Option<PathBuf>,
    prompt: String,
    print_only: bool,
    yes: bool,
    use_cloud: bool,
) -> Result<()> {
    let cfg = load_cfg(config_path.clone())?;
    let reg = ModelRegistry::load_merged(&resolve_registry_cache_path_for_read())?;
    let host = HostContext::gather(
        cfg.preferred_shell.as_deref(),
        cfg.execution_profile.as_deref(),
    );
    let system = build_system_prompt(&host);
    let user = format!("User request: {}\nReply with ONLY the JSON object.", prompt);

    let raw = if use_cloud && cfg.cloud.enabled {
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
        engine::complete_local_best_effort(&path, &system, &user, 256)?
    };

    let proposal = CommandProposal::parse_from_model_text(&raw)?;
    println!("Proposed: {}", serde_json::to_string_pretty(&proposal)?);

    let jail = std::env::current_dir()?;
    let policy = PolicyEngine::new(
        jail,
        cfg.policy.strict_allowlist,
        cfg.policy.allowlist_bins.clone(),
    );
    let decision = policy.evaluate(&proposal);
    if decision.blocked {
        return Err(clai::AppError::Msg(
            decision
                .reason
                .unwrap_or_else(|| "blocked by policy".into()),
        ));
    }

    if print_only {
        println!("(print-only; not executed)");
        return Ok(());
    }

    if decision.requires_confirmation && !yes {
        let ok = inquire::Confirm::new("This command is sensitive or destructive. Run it?")
            .with_default(false)
            .prompt()
            .map_err(|e| clai::AppError::Msg(e.to_string()))?;
        if !ok {
            println!("Aborted.");
            return Ok(());
        }
    }

    if cfg.policy.dry_run_default && !yes {
        println!("(dry-run; not executed)");
        return Ok(());
    }

    let out = executor::run_proposal(
        &proposal,
        Duration::from_secs(120),
        256 * 1024,
        &cfg.execution,
    )?;
    println!(
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status, out.stdout, out.stderr
    );
    Ok(())
}

fn cmd_models(config_path: Option<PathBuf>, m: ModelsCmd) -> Result<()> {
    let cache_read = resolve_registry_cache_path_for_read();
    let cache_write = default_registry_cache_path();
    match m {
        ModelsCmd::List => {
            let reg = ModelRegistry::load_merged(&cache_read)?;
            let cfg = config::load_config_raw(config_path.clone()).unwrap_or_default();
            for m in &reg.models {
                let mark = cfg
                    .default_model_id
                    .as_deref()
                    .map(|d| d == m.id)
                    .unwrap_or(false);
                let loc = installed_model_path(&m.filename)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(not downloaded)".into());
                println!(
                    "{}{} — {} [{}] {}",
                    if mark { "* " } else { "  " },
                    m.id,
                    m.display_name,
                    m.profile,
                    loc
                );
            }
        }
        ModelsCmd::Search { query } => {
            let reg = ModelRegistry::load_merged(&cache_read)?;
            for m in reg.search(&query) {
                println!("{} — {}", m.id, m.display_name);
            }
        }
        ModelsCmd::Pull { id, verify } => {
            let reg = ModelRegistry::load_merged(&cache_read)?;
            let m = reg
                .find(&id)
                .ok_or_else(|| clai::AppError::Msg(format!("unknown model {}", id)))?;
            let p = registry::pull_model(m, &default_models_dir(), verify)?;
            println!("{}", p.display());
        }
        ModelsCmd::Default { action } => {
            let mut cfg = load_cfg(config_path.clone()).unwrap_or_default();
            match action {
                DefaultModelCmd::Set { id } => {
                    cfg.default_model_id = Some(id);
                    cfg.save(config_path)?;
                }
            }
        }
        ModelsCmd::Rm { id } => {
            let reg = ModelRegistry::load_merged(&cache_read)?;
            let m = reg
                .find(&id)
                .ok_or_else(|| clai::AppError::Msg(format!("unknown model {}", id)))?;
            let p = installed_model_path(&m.filename).ok_or_else(|| {
                clai::AppError::Msg(format!("model file not present: {}", id))
            })?;
            std::fs::remove_file(&p)?;
            println!("removed {}", p.display());
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
            println!(
                "wrote {} (version {})",
                cache_write.display(),
                reg.registry_version
            );
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
        MigrateCmd::DryRun => println!("{}", migrate::dry_run(config_path)?),
        MigrateCmd::Apply => migrate::apply(config_path)?,
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
         If you refuse, use program \"echo\" and args [\"refused\"] and a reason field.",
        CommandProposal::schema_json(),
        host.os,
        host.arch,
        host.cwd,
        host.shell_family,
        host.path_separator
    )
}
