use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("config: {0}")]
    Config(#[from] figment::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),

    #[error("{0}")]
    Msg(String),

    #[error("model registry major version {got} unsupported; upgrade clai")]
    RegistryVersion { got: u32 },

    #[error("config version {current} requires migration to {latest}; run `clai migrate --dry-run`")]
    ConfigNeedsMigration { current: u32, latest: u32 },
}

pub type Result<T> = std::result::Result<T, AppError>;
