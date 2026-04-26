//! Model registry: embedded defaults + cached updates.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use hf_hub::api::sync::Api;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{default_models_dir, installed_model_path};
use crate::error::{AppError, Result};

pub const EMBEDDED_REGISTRY: &str = include_str!("../assets/registry.json");
/// Refuse registry.json with version greater than this (format bump).
pub const MAX_SUPPORTED_REGISTRY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub registry_version: u32,
    pub models: Vec<RegistryModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryModel {
    pub id: String,
    pub display_name: String,
    pub profile: String,
    pub hf_repo: String,
    pub filename: String,
    pub sha256: Option<String>,
    #[serde(default)]
    pub ram_hint_gb: Option<u32>,
}

impl ModelRegistry {
    pub fn embedded() -> Result<Self> {
        serde_json::from_str(EMBEDDED_REGISTRY).map_err(Into::into)
    }

    pub fn load_path(path: &Path) -> Result<Self> {
        let s = fs::read_to_string(path)?;
        serde_json::from_str(&s).map_err(Into::into)
    }

    pub fn load_merged(cache_path: &Path) -> Result<Self> {
        let mut base = Self::embedded()?;
        if cache_path.exists() {
            let cached = Self::load_path(cache_path)?;
            if cached.registry_version > MAX_SUPPORTED_REGISTRY_VERSION {
                return Err(AppError::RegistryVersion {
                    got: cached.registry_version,
                });
            }
            if cached.registry_version > base.registry_version {
                base = cached;
            } else if cached.registry_version == base.registry_version {
                // merge: prefer cached models by id override
                for m in cached.models {
                    if let Some(idx) = base.models.iter().position(|x| x.id == m.id) {
                        base.models[idx] = m;
                    } else {
                        base.models.push(m);
                    }
                }
            }
        }
        Ok(base)
    }

    pub fn find(&self, id: &str) -> Option<&RegistryModel> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn search(&self, query: &str) -> Vec<&RegistryModel> {
        let tokens: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            return self.models.iter().collect();
        }
        self.models
            .iter()
            .filter(|m| {
                let hay = format!("{} {} {}", m.id, m.display_name, m.profile).to_lowercase();
                tokens.iter().all(|t| hay.contains(t))
            })
            .collect()
    }
}

pub fn pull_model(model: &RegistryModel, dest_dir: &Path, verify: bool) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir)?;
    let out_path = dest_dir.join(&model.filename);
    if out_path.exists() {
        if let Some(expected) = &model.sha256 {
            if verify && !hash_matches(&out_path, expected)? {
                fs::remove_file(&out_path)?;
            } else {
                return Ok(out_path);
            }
        } else {
            return Ok(out_path);
        }
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {wide_msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(format!("downloading {} via Hugging Face", model.filename));

    let api = Api::new().map_err(|e| AppError::Msg(e.to_string()))?;
    let repo = api.model(model.hf_repo.clone());
    let cached = repo
        .get(&model.filename)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    pb.finish_with_message("copying to model dir");
    fs::copy(&cached, &out_path)?;

    if let Some(expected) = &model.sha256 {
        if verify && !hash_matches(&out_path, expected)? {
            fs::remove_file(&out_path)?;
            return Err(AppError::Msg("sha256 mismatch after download".into()));
        }
    }

    Ok(out_path)
}

fn hash_matches(path: &Path, expected_hex: &str) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let out = hex::encode(hasher.finalize());
    Ok(out.eq_ignore_ascii_case(expected_hex))
}

pub fn default_model_path_for(id: &str, registry: &ModelRegistry) -> Result<PathBuf> {
    let m = registry
        .find(id)
        .ok_or_else(|| AppError::Msg(format!("unknown model id {}", id)))?;
    installed_model_path(&m.filename).ok_or_else(|| {
        AppError::Msg(format!(
            "model file not found; run: clai models pull {} (installs to {})",
            id,
            default_models_dir().join(&m.filename).display()
        ))
    })
}

pub fn write_registry_cache(path: &Path, reg: &ModelRegistry) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(reg)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_parses() {
        let r = ModelRegistry::embedded().unwrap();
        assert_eq!(r.registry_version, 1);
        assert!(!r.models.is_empty());
    }

    #[test]
    fn search_finds_qwen() {
        let r = ModelRegistry::embedded().unwrap();
        let hits = r.search("qwen 7b");
        assert!(!hits.is_empty());
    }
}
