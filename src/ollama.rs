//! Optional discovery of models installed in a local Ollama daemon (separate from clai GGUF pull).

use serde::Deserialize;

use crate::error::{AppError, Result};

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelRow>,
}

#[derive(Debug, Deserialize)]
struct ModelRow {
    name: String,
    #[serde(default)]
    size: Option<u64>,
}

/// Returns model names (tags) from `GET {base}/api/tags`.
pub fn list_local_tags(base_url: &str) -> Result<Vec<(String, Option<u64>)>> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/tags");
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| AppError::Msg(format!("ollama {url}: {e}")))?;
    let status = resp.status();
    if status != 200 {
        return Err(AppError::Msg(format!("ollama {url}: HTTP {status}",)));
    }
    let body = resp
        .into_string()
        .map_err(|e| AppError::Msg(format!("ollama response body: {e}")))?;
    let parsed: TagsResponse =
        serde_json::from_str(&body).map_err(|e| AppError::Msg(format!("ollama JSON: {e}")))?;
    Ok(parsed
        .models
        .into_iter()
        .map(|m| (m.name, m.size))
        .collect())
}
