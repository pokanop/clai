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
        }
    }
}

impl AppConfig {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let path = path.unwrap_or_else(default_config_path);
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

pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clai")
        .join("config.toml")
}

pub fn default_models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clai")
        .join("models")
}

pub fn default_registry_cache_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clai")
        .join("registry.json")
}

/// Merge file + env without migration guard (used by `migrate` subcommand).
pub fn load_config_raw(path: Option<PathBuf>) -> Result<AppConfig> {
    let path = path.unwrap_or_else(default_config_path);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Figment::new()
        .merge(Toml::file(&path))
        .merge(Env::prefixed("CLAI_").split("__"))
        .extract()
        .map_err(AppError::Config)
}
