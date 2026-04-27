//! Bounded PATH probes for common interpreters (FR-1, NFR-1, NFR-2).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;

/// Cached snapshot for the process when detection is enabled (NFR-1).
static CACHED_TOOLING: OnceLock<RuntimeTooling> = OnceLock::new();

fn empty_tooling() -> RuntimeTooling {
    RuntimeTooling {
        python3: None,
        python: None,
        node: None,
        ruby: None,
        perl: None,
        php: None,
    }
}

/// Resolved tooling on the host: each field is `Some(absolute or usable path)` when invokable.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeTooling {
    pub python3: Option<String>,
    pub python: Option<String>,
    pub node: Option<String>,
    pub ruby: Option<String>,
    pub perl: Option<String>,
    pub php: Option<String>,
}

impl RuntimeTooling {
    /// JSON for the system prompt (compact, stable keys).
    pub fn to_prompt_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Human lines for `clai doctor`: `(label, status)`.
    pub fn doctor_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("python3", status(&self.python3)),
            ("python", status(&self.python)),
            ("node", status(&self.node)),
            ("ruby", status(&self.ruby)),
            ("perl", status(&self.perl)),
            ("php", status(&self.php)),
        ]
    }
}

fn status(path: &Option<String>) -> String {
    match path {
        Some(p) => format!("yes — {p}"),
        None => "not found on PATH".into(),
    }
}

/// Return tooling for prompts and doctor. When `detect` is false, returns an empty snapshot without probing.
#[must_use]
pub fn runtime_tooling_snapshot(detect: bool) -> RuntimeTooling {
    if !detect {
        return empty_tooling();
    }
    CACHED_TOOLING.get_or_init(probe_visible_tools).clone()
}

fn probe_visible_tools() -> RuntimeTooling {
    RuntimeTooling {
        python3: resolve_on_path_candidates(&[OsStr::new("python3")]),
        python: resolve_on_path_candidates(&[OsStr::new("python"), OsStr::new("py")]),
        node: resolve_on_path_candidates(&[OsStr::new("node")]),
        ruby: resolve_on_path_candidates(&[OsStr::new("ruby")]),
        perl: resolve_on_path_candidates(&[OsStr::new("perl")]),
        php: resolve_on_path_candidates(&[OsStr::new("php")]),
    }
}

fn resolve_on_path_candidates(names: &[&OsStr]) -> Option<String> {
    for n in names {
        if let Some(p) = resolve_on_path(n) {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

fn resolve_on_path(name: &OsStr) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_usable_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            for ext in ["exe", "bat", "cmd"] {
                let with_ext = candidate.with_extension(ext);
                if is_usable_executable(&with_ext) {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}

fn is_usable_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if cfg!(unix) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    } else {
        // Windows: existence as a file in PATH is the usual heuristic.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_when_detect_off() {
        let t = runtime_tooling_snapshot(false);
        assert_eq!(t.python3, None);
        assert_eq!(t.node, None);
    }

    #[test]
    fn snapshot_is_serializable() {
        let t = RuntimeTooling {
            python3: Some("/usr/bin/python3".into()),
            python: None,
            node: None,
            ruby: None,
            perl: None,
            php: None,
        };
        let j = t.to_prompt_json();
        assert!(j.contains("python3"));
    }
}
