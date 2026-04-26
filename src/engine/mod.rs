//! Local inference: embedded llama.cpp when `llama` feature is enabled.

#[cfg(feature = "llama")]
mod llama;

#[cfg(feature = "llama")]
pub use llama::complete_local;

use crate::error::{AppError, Result};

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
