//! Optional OpenAI-compatible HTTP client for cloud fallback.

use serde_json::json;

use crate::error::{AppError, Result};
use crate::schema::CommandProposal;

pub fn complete_cloud(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    );
    let body = json!({
        "model": model,
        "temperature": 0.0,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ]
    });

    let mut req = ureq::post(&url).set("Content-Type", "application/json");
    if let Some(k) = api_key {
        req = req.set("Authorization", &format!("Bearer {}", k));
    }
    let resp = req
        .send_json(body)
        .map_err(|e| AppError::Msg(format!("cloud request failed: {}", e)))?;

    let v: serde_json::Value = resp
        .into_json()
        .map_err(|e| AppError::Msg(format!("cloud json: {}", e)))?;
    let text = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| AppError::Msg("cloud: missing choices[0].message.content".into()))?;
    Ok(text.to_string())
}

pub fn cloud_proposal(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system: &str,
    user_prompt: &str,
) -> Result<CommandProposal> {
    let raw = complete_cloud(base_url, api_key, model, system, user_prompt)?;
    CommandProposal::parse_from_model_text(&raw)
}
