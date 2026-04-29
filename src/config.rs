use std::path::{Path, PathBuf};

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::interactive_history::DEFAULT_HISTORY_MAX_ENTRIES;
use crate::interactive_mode::InteractiveExecutionMode;

/// Bump when automatic migrations are required.
pub const CONFIG_VERSION_LATEST: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    #[default]
    Direct,
    Docker,
    Bwrap,
}

/// Optional sandbox / wrapper for `run` (see `execution.mode`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionConfig {
    #[serde(default)]
    pub mode: ExecutionMode,
    /// Docker image when `mode = "docker"` (must contain your CLI tools or use a custom image).
    #[serde(default)]
    pub docker_image: Option<String>,
    #[serde(default)]
    pub docker_extra_args: Vec<String>,
    /// Extra `bwrap` args before `--` when `mode = "bwrap"`.
    #[serde(default)]
    pub bwrap_extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_config_version")]
    pub config_version: u32,

    #[serde(default)]
    pub default_model_id: Option<String>,

    #[serde(default)]
    pub model_path: Option<PathBuf>,

    /// posix | windows_powershell | windows_cmd
    #[serde(default)]
    pub execution_profile: Option<String>,

    #[serde(default)]
    pub preferred_shell: Option<String>,

    #[serde(default)]
    pub execution: ExecutionConfig,

    #[serde(default)]
    pub policy: PolicyConfig,

    #[serde(default)]
    pub cloud: CloudConfig,

    /// When true, `clai ask` uses machine-oriented output (full proposal + captured streams) unless
    /// overridden on the command line. CLI `--verbose` / `CLAI_ASK_VERBOSE` also enables this.
    #[serde(default)]
    pub ask_verbose: bool,
    /// When true, `clai ask` uses piped capture in direct mode even on a TTY (see `--force-capture`).
    #[serde(default)]
    pub ask_force_capture: bool,
    /// When true, do not print the one-line pre-run preview (`Run:` or non-direct context) for human
    /// default output (see `--no-preview`).
    #[serde(default)]
    pub ask_no_preview: bool,

    /// Default interactive session execution behavior (overridable by env / CLI; see README).
    #[serde(default)]
    pub interactive: InteractiveSection,

    /// Extra Hugging Face GGUF catalog entries (merged with the built-in and cached registry).
    #[serde(default)]
    pub models: ModelsSection,

    /// Runtime detection and ephemeral script materialization (see PRD adaptive script execution).
    #[serde(default)]
    pub tooling: ToolingConfig,
}

/// `[tooling]` — PATH probes and optional file-backed scripts from model output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolingConfig {
    /// When true (default), probe PATH once per process and expose results in the system prompt and `doctor`.
    #[serde(default = "default_true")]
    pub detect_runtimes: bool,
    /// When true, `script_body` in model JSON is written to a private temp file and the path is appended to argv.
    #[serde(default)]
    pub ephemeral_scripts: bool,
    /// Prompt guidance: prefer short scripts when a runtime fits (does not force model output).
    #[serde(default = "default_true")]
    pub prefer_scripts_when_available: bool,
}

impl Default for ToolingConfig {
    fn default() -> Self {
        Self {
            detect_runtimes: true,
            ephemeral_scripts: false,
            prefer_scripts_when_available: true,
        }
    }
}

/// Optional Hugging Face GGUF entry; same fields as `registry.json` model objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraModelEntry {
    pub id: String,
    pub display_name: String,
    pub profile: String,
    pub hf_repo: String,
    pub filename: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub ram_hint_gb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelsSection {
    #[serde(default)]
    pub extra: Vec<ExtraModelEntry>,
}

/// When to load the local GGUF in an interactive **local** session (see README).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LocalWarmupMode {
    /// Lazy load on the first line that runs local inference (historical default).
    #[default]
    Off,
    /// Load the model after the session banner, before the first `clai>` prompt.
    Blocking,
}

/// Config table `[interactive]` / env `CLAI_INTERACTIVE__EXECUTION`, `CLAI_INTERACTIVE__LOCAL_WARMUP`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveSection {
    /// When set, authoritative for interactive mode when no CLI override applies.
    #[serde(default)]
    pub execution: Option<InteractiveExecutionMode>,
    /// Optional eager load of the GGUF at session start in local + embedded-llama mode. Default: off
    /// until product benchmarks; use `off` for low-memory machines or non-interactive automation.
    #[serde(default)]
    pub local_warmup: LocalWarmupMode,
    /// Interactive **confirm** mode only: basenames that skip the “Run proposed command?” prompt
    /// (policy still applies). Populated via session “remember” prompts or by hand in config.
    #[serde(default)]
    pub remember_run_programs: Vec<String>,
    /// Max qualifying request lines recalled with Up; minimum **100**, default **1000** (`[interactive]` / `CLAI_INTERACTIVE__HISTORY_MAX_ENTRIES`).
    #[serde(default = "interactive_history_cap_default")]
    pub history_max_entries: usize,
}

fn interactive_history_cap_default() -> usize {
    DEFAULT_HISTORY_MAX_ENTRIES
}

impl Default for InteractiveSection {
    fn default() -> Self {
        Self {
            execution: None,
            local_warmup: LocalWarmupMode::default(),
            remember_run_programs: Vec::new(),
            history_max_entries: DEFAULT_HISTORY_MAX_ENTRIES,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyConfig {
    #[serde(default = "default_true")]
    pub dry_run_default: bool,
    #[serde(default)]
    pub allowlist_bins: Vec<String>,
    #[serde(default)]
    pub strict_allowlist: bool,
    /// Program basenames that skip the extra policy confirmation when the command is otherwise allowed
    /// (not blocked, not `needs_shell`, not destructive). Project `clai.toml` can extend the global list.
    #[serde(default)]
    pub trusted_programs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Set false if the server rejects `response_format.type = json_schema`.
    #[serde(default = "default_true")]
    pub structured_outputs: bool,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: None,
            api_key_env: None,
            model: None,
            structured_outputs: true,
        }
    }
}

fn default_config_version() -> u32 {
    CONFIG_VERSION_LATEST
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CONFIG_VERSION_LATEST,
            default_model_id: None,
            model_path: None,
            execution_profile: None,
            preferred_shell: None,
            execution: ExecutionConfig::default(),
            policy: PolicyConfig {
                dry_run_default: true,
                allowlist_bins: vec![],
                strict_allowlist: false,
                trusted_programs: vec![],
            },
            cloud: CloudConfig::default(),
            ask_verbose: false,
            ask_force_capture: false,
            ask_no_preview: false,
            interactive: InteractiveSection::default(),
            models: ModelsSection::default(),
            tooling: ToolingConfig::default(),
        }
    }
}

/// Walk from `start` (usually cwd) up to root; collect `clai.toml` and `.clai/config.toml`.
/// Merge order is outer directories first, then inner — last file wins (closest to cwd).
pub fn discover_local_config_paths(start: &Path) -> Vec<PathBuf> {
    let mut stack = Vec::new();
    let mut cur = start.to_path_buf();
    loop {
        for rel in [".clai/config.toml", "clai.toml"] {
            let p = cur.join(rel);
            if p.is_file() {
                stack.push(p);
            }
        }
        if !cur.pop() {
            break;
        }
    }
    stack.reverse();
    stack
}

fn config_figment_layers(global_file: Option<PathBuf>, local_files: Vec<PathBuf>) -> Figment {
    let mut f = Figment::new();
    if let Some(g) = global_file {
        if g.is_file() {
            f = f.merge(Toml::file(g));
        }
    }
    for p in local_files {
        if p.is_file() {
            f = f.merge(Toml::file(p));
        }
    }
    f.merge(Env::prefixed("CLAI_").split("__"))
}

fn build_config_figment(cli_override: Option<PathBuf>) -> Result<Figment> {
    if let Some(p) = cli_override {
        if !p.is_file() {
            return Ok(Figment::new().merge(Env::prefixed("CLAI_").split("__")));
        }
        return Ok(Figment::new()
            .merge(Toml::file(&p))
            .merge(Env::prefixed("CLAI_").split("__")));
    }
    let global = resolve_config_path_for_read(None);
    let global_opt = global.is_file().then_some(global);
    let cwd = std::env::current_dir().map_err(|e| AppError::Msg(e.to_string()))?;
    let locals = discover_local_config_paths(&cwd);
    Ok(config_figment_layers(global_opt, locals))
}

impl AppConfig {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let figment = build_config_figment(path)?;
        let c: AppConfig = figment.extract().map_err(AppError::Config)?;
        if c.config_version > CONFIG_VERSION_LATEST {
            return Err(AppError::Msg(format!(
                "config version {} is newer than this clai supports ({})",
                c.config_version, CONFIG_VERSION_LATEST
            )));
        }
        if c.config_version < CONFIG_VERSION_LATEST {
            return Err(AppError::ConfigNeedsMigration {
                current: c.config_version,
                latest: CONFIG_VERSION_LATEST,
            });
        }
        Ok(c)
    }

    pub fn save(&self, path: Option<PathBuf>) -> Result<()> {
        let path = path.unwrap_or_else(default_config_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = toml::to_string_pretty(self)?;
        std::fs::write(path, s)?;
        Ok(())
    }
}

/// Preferred config path: XDG-style `~/.config/clai/config.toml` on Unix (including macOS),
/// or `dirs::config_dir()` on Windows.
pub fn default_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clai")
            .join("config.toml")
    }
    #[cfg(not(target_os = "windows"))]
    {
        xdg_config_home()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clai")
            .join("config.toml")
    }
}

#[cfg(not(target_os = "windows"))]
fn xdg_config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
}

#[cfg(not(target_os = "windows"))]
fn xdg_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
}

/// Application data root: XDG-style `~/.local/share/clai` on Unix (including macOS),
/// or `%LOCALAPPDATA%\\clai` on Windows.
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clai")
    }
    #[cfg(not(target_os = "windows"))]
    {
        xdg_data_home()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clai")
    }
}

#[cfg(target_os = "macos")]
fn legacy_macos_clai_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Application Support/clai")
}

/// Config file to read when `path` is `None`: preferred path, else legacy macOS
/// `Library/Application Support` if that file exists (older clai releases).
fn resolve_config_path_for_read(path: Option<PathBuf>) -> PathBuf {
    if let Some(p) = path {
        return p;
    }
    let preferred = default_config_path();
    if preferred.exists() {
        return preferred;
    }
    #[cfg(target_os = "macos")]
    {
        let legacy = legacy_macos_clai_dir().join("config.toml");
        if legacy.exists() {
            return legacy;
        }
    }
    preferred
}

pub fn default_models_dir() -> PathBuf {
    default_data_dir().join("models")
}

pub fn default_registry_cache_path() -> PathBuf {
    default_data_dir().join("registry.json")
}

/// Registry cache to read when using the default location: preferred data dir first,
/// then legacy macOS `Application Support/clai/registry.json`.
pub fn resolve_registry_cache_path_for_read() -> PathBuf {
    let preferred = default_registry_cache_path();
    if preferred.exists() {
        return preferred;
    }
    #[cfg(target_os = "macos")]
    {
        let legacy = legacy_macos_clai_dir().join("registry.json");
        if legacy.exists() {
            return legacy;
        }
    }
    preferred
}

/// Resolved path to a downloaded model file, if it exists in the preferred or legacy tree.
pub fn installed_model_path(filename: &str) -> Option<PathBuf> {
    let preferred = default_models_dir().join(filename);
    if preferred.exists() {
        return Some(preferred);
    }
    #[cfg(target_os = "macos")]
    {
        let legacy = legacy_macos_clai_dir().join("models").join(filename);
        if legacy.exists() {
            return Some(legacy);
        }
    }
    None
}

/// Merge file + env without migration guard (used by `migrate` subcommand).
pub fn load_config_raw(path: Option<PathBuf>) -> Result<AppConfig> {
    build_config_figment(path)?
        .extract()
        .map_err(AppError::Config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive_mode::InteractiveExecutionMode;
    use std::io::Write;

    #[test]
    fn local_warmup_parses_from_toml_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&p).expect("file");
        f.write_all(b"config_version = 1\n[interactive]\nlocal_warmup = \"blocking\"\n")
            .expect("write");
        let c = AppConfig::load(Some(p)).expect("load");
        assert_eq!(c.interactive.local_warmup, LocalWarmupMode::Blocking);
    }

    #[test]
    fn default_local_warmup_is_off() {
        assert_eq!(
            AppConfig::default().interactive.local_warmup,
            LocalWarmupMode::Off
        );
    }

    #[test]
    fn interactive_history_max_entries_default_and_parse() {
        assert_eq!(
            AppConfig::default().interactive.history_max_entries,
            crate::interactive_history::DEFAULT_HISTORY_MAX_ENTRIES
        );
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("config.toml");
        std::fs::write(
            p.as_path(),
            b"config_version = 1\n[interactive]\nhistory_max_entries = 250\n",
        )
        .expect("write");
        let c = AppConfig::load(Some(p)).expect("load");
        assert_eq!(c.interactive.history_max_entries, 250);
    }

    #[test]
    fn tooling_section_parses_from_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&p).expect("file");
        f.write_all(
            b"config_version = 1\n[tooling]\ndetect_runtimes = false\nephemeral_scripts = true\nprefer_scripts_when_available = false\n",
        )
        .expect("write");
        let c = AppConfig::load(Some(p)).expect("load");
        assert!(!c.tooling.detect_runtimes);
        assert!(c.tooling.ephemeral_scripts);
        assert!(!c.tooling.prefer_scripts_when_available);
    }

    #[test]
    fn interactive_remember_run_programs_parses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("config.toml");
        std::fs::write(
            p.as_path(),
            b"config_version = 1\n[interactive]\nexecution = \"confirm\"\nremember_run_programs = [\"ls\"]\n",
        )
        .expect("write");
        let c = AppConfig::load(Some(p)).expect("load");
        assert_eq!(c.interactive.remember_run_programs, vec!["ls".to_string()]);
    }

    #[test]
    fn local_clai_toml_overrides_global_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let global = dir.path().join("global.toml");
        std::fs::write(
            global.as_path(),
            b"config_version = 1\n[interactive]\nexecution = \"dry-run\"\n",
        )
        .expect("write global");
        let local = dir.path().join("clai.toml");
        std::fs::write(local.as_path(), b"[interactive]\nexecution = \"auto\"\n").expect("local");
        let f = config_figment_layers(Some(global), vec![local]);
        let c: AppConfig = f.extract().expect("extract");
        assert_eq!(
            c.interactive.execution,
            Some(InteractiveExecutionMode::Auto)
        );
    }

    #[test]
    fn discover_local_config_paths_orders_ancestor_before_descendant() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("root");
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).expect("mkdir");
        std::fs::write(
            root.join("clai.toml"),
            b"[interactive]\nexecution = \"dry-run\"\n",
        )
        .unwrap();
        std::fs::write(
            sub.join("clai.toml"),
            b"[interactive]\nexecution = \"auto\"\n",
        )
        .unwrap();
        let paths = discover_local_config_paths(&sub);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], root.join("clai.toml"));
        assert_eq!(paths[1], sub.join("clai.toml"));
    }
}
