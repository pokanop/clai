//! Local inference: embedded llama.cpp when `llama` feature is enabled.

#[cfg(feature = "llama")]
mod llama;

#[cfg(feature = "llama")]
pub use llama::{complete_local, complete_local_with, LocalLlamaSession};

use crate::error::{AppError, Result};

/// Default cap on generated tokens for local NL→command (`ask`, interactive). Small limits truncate
/// long `args` or `reason` text and produce unparseable JSON; override with `CLAI_MAX_NEW_TOKENS`.
pub const DEFAULT_MAX_NEW_TOKENS: i32 = 4096;

/// Effective max new tokens for local completion, from `CLAI_MAX_NEW_TOKENS` (positive, capped) or
/// [`DEFAULT_MAX_NEW_TOKENS`].
pub fn max_new_tokens_local() -> i32 {
    const CAP: i32 = 32_768;
    std::env::var("CLAI_MAX_NEW_TOKENS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .filter(|&n| n > 0)
        .map(|n| n.min(CAP))
        .unwrap_or(DEFAULT_MAX_NEW_TOKENS)
}

/// Generate model text for NL→command (local or error if disabled).
pub fn complete_local_best_effort(
    model_path: &std::path::Path,
    system: &str,
    user: &str,
    max_tokens: i32,
) -> Result<String> {
    #[cfg(feature = "llama")]
    {
        complete_local(model_path, system, user, max_tokens).map_err(AppError::Msg)
    }
    #[cfg(not(feature = "llama"))]
    {
        let _ = (model_path, system, user, max_tokens);
        Err(AppError::Msg(
            "clai was built without `llama`; rebuild with default features".into(),
        ))
    }
}
