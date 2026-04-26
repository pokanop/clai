//! Config migrations keyed by `config_version`.

use crate::config::{load_config_raw, AppConfig, CONFIG_VERSION_LATEST};
use crate::error::Result;

pub fn dry_run(path: Option<std::path::PathBuf>) -> Result<String> {
    let c = load_config_raw(path)?;
    let mut lines = vec![format!("current config_version: {}", c.config_version)];
    if c.config_version >= CONFIG_VERSION_LATEST {
        lines.push("no migration needed.".into());
        return Ok(lines.join("\n"));
    }
    let mut v = c.config_version;
    while v < CONFIG_VERSION_LATEST {
        let next = v + 1;
        lines.push(format!("would migrate {} -> {}", v, next));
        v = next;
    }
    Ok(lines.join("\n"))
}

pub fn apply(path: Option<std::path::PathBuf>) -> Result<()> {
    let mut c = load_config_raw(path.clone())?;
    while c.config_version < CONFIG_VERSION_LATEST {
        let next = c.config_version + 1;
        apply_step(&mut c, next)?;
    }
    c.save(path)
}

fn apply_step(c: &mut AppConfig, target: u32) -> Result<()> {
    match (c.config_version, target) {
        (0, 1) => {
            c.config_version = 1;
            Ok(())
        }
        _ => {
            c.config_version = target;
            Ok(())
        }
    }
}
