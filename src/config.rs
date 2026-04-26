use std::path::PathBuf;

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyConfig {
    #[serde(default = "default_true")]
    pub dry_run_default: bool,
    #[serde(default)]
    pub allowlist_bins: Vec<String>,
    #[serde(default)]
    pub strict_allowlist: bool,
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
            },
            cloud: CloudConfig::default(),
            ask_verbose: false,
        }
    }
}

impl AppConfig {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let path = resolve_config_path_for_read(path);
        if !path.exists() {
            return Ok(Self::default());
        }
        let figment = Figment::new()
            .merge(Toml::file(&path))
            .merge(Env::prefixed("CLAI_").split("__"));
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
    let path = resolve_config_path_for_read(path);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Figment::new()
        .merge(Toml::file(&path))
        .merge(Env::prefixed("CLAI_").split("__"))
        .extract()
        .map_err(AppError::Config)
}
