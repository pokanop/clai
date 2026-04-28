//! Persist user-approved executables from interactive prompts: `[policy].trusted_programs` and
//! `[interactive].remember_run_programs` (confirm-mode run prompt skip).

use std::fmt;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use inquire::Select;

use crate::config::{default_config_path, CONFIG_VERSION_LATEST};
use crate::error::{AppError, Result};
use crate::tty::{println_labeled, Severity};

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrustScope {
    Project,
    Global,
}

impl fmt::Display for TrustScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project => write!(
                f,
                "This project (clai.toml or .clai/config.toml in this directory)"
            ),
            Self::Global => write!(f, "Global user config"),
        }
    }
}

#[must_use]
pub fn program_basename(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
        .to_string()
}

#[must_use]
pub fn trusted_list_contains(trusted: &[String], basename: &str) -> bool {
    trusted.iter().any(|t| t.eq_ignore_ascii_case(basename))
}

/// Prefer an existing project config in `cwd`; otherwise default to `./clai.toml`.
#[must_use]
pub fn resolve_project_trusted_write_path(cwd: &Path) -> PathBuf {
    let clai = cwd.join("clai.toml");
    let dot = cwd.join(".clai/config.toml");
    if clai.is_file() {
        clai
    } else if dot.is_file() {
        dot
    } else {
        clai
    }
}

fn append_basename_to_nested_array(
    path: &Path,
    section: &str,
    key: &str,
    program: &str,
) -> Result<bool> {
    let name = program_basename(program);
    let mut root = if path.exists() {
        let raw = std::fs::read_to_string(path)?;
        raw.parse::<toml::Value>()
            .map_err(|e| AppError::Msg(format!("{}: {e}", path.display())))?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let table = root
        .as_table_mut()
        .ok_or_else(|| AppError::Msg("config root must be a table".into()))?;

    if !path.exists() {
        table.insert(
            "config_version".to_string(),
            toml::Value::Integer(i64::from(CONFIG_VERSION_LATEST)),
        );
    }

    let sec = table
        .entry(section.to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let sec_table = sec
        .as_table_mut()
        .ok_or_else(|| AppError::Msg(format!("[{section}] must be a table")))?;

    let arr_val = sec_table
        .entry(key.to_string())
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    let arr = arr_val
        .as_array_mut()
        .ok_or_else(|| AppError::Msg(format!("{key} must be an array")))?;

    if arr
        .iter()
        .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(&name)))
    {
        return Ok(false);
    }

    arr.push(toml::Value::String(name));

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let out = toml::to_string_pretty(&root)?;
    std::fs::write(path, out)?;
    Ok(true)
}

/// Append `program`'s basename to `trusted_programs` in a TOML file (creates a minimal file if missing).
/// Returns `true` if the file was updated.
pub fn append_trusted_program(path: &Path, program: &str) -> Result<bool> {
    append_basename_to_nested_array(path, "policy", "trusted_programs", program)
}

/// Append basename to `[interactive].remember_run_programs` (interactive confirm mode memory).
pub fn append_remember_run_program(path: &Path, program: &str) -> Result<bool> {
    append_basename_to_nested_array(path, "interactive", "remember_run_programs", program)
}

/// After the user confirmed a policy-gated run, offer to persist the executable to `trusted_programs`.
///
/// `needs_shell` proposals are ignored: trust entries do not bypass shell confirmation, so we do not offer.
pub fn prompt_and_append_trusted_if_desired(
    trusted: &mut Vec<String>,
    program: &str,
    needs_shell: bool,
) -> Result<()> {
    if needs_shell {
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(());
    }

    let base = program_basename(program);
    if trusted_list_contains(trusted, &base) {
        return Ok(());
    }

    let remember = match inquire::Confirm::new(&format!(
        "Add `{base}` to trusted programs (skip policy confirmation for this executable later)?"
    ))
    .with_default(false)
    .prompt()
    {
        Ok(v) => v,
        Err(e) => {
            println_labeled(
                "clai",
                &format!("could not read choice: {e}"),
                Severity::Warn,
            );
            return Ok(());
        }
    };

    if !remember {
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let proj_path = resolve_project_trusted_write_path(&cwd);
    let global_path = default_config_path();

    let scope = match Select::new(
        "Save trusted program to:",
        vec![TrustScope::Project, TrustScope::Global],
    )
    .prompt()
    {
        Ok(v) => v,
        Err(e) => {
            println_labeled(
                "clai",
                &format!("could not read choice: {e}"),
                Severity::Warn,
            );
            return Ok(());
        }
    };

    let target = match scope {
        TrustScope::Project => proj_path,
        TrustScope::Global => global_path,
    };

    let wrote = append_trusted_program(&target, program)?;
    if wrote {
        println_labeled(
            "clai",
            &format!(
                "Wrote `{base}` to [policy].trusted_programs in {}.",
                target.display()
            ),
            Severity::Ok,
        );
    }
    if !trusted_list_contains(trusted, &base) {
        trusted.push(base);
    }
    Ok(())
}

/// After the user confirmed the interactive **run** step in confirm mode, offer to skip that prompt later.
///
/// Interactive sessions only (`[interactive].remember_run_programs`). `needs_shell` proposals are ignored.
pub fn prompt_and_append_remember_run_if_desired(
    remembered: &mut Vec<String>,
    program: &str,
    needs_shell: bool,
) -> Result<()> {
    if needs_shell {
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(());
    }

    let base = program_basename(program);
    if trusted_list_contains(remembered, &base) {
        return Ok(());
    }

    let remember = match inquire::Confirm::new(&format!(
        "Remember `{base}` so confirm mode skips the \"Run proposed command?\" step for it next time?"
    ))
    .with_default(false)
    .prompt()
    {
        Ok(v) => v,
        Err(e) => {
            println_labeled(
                "clai",
                &format!("could not read choice: {e}"),
                Severity::Warn,
            );
            return Ok(());
        }
    };

    if !remember {
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let proj_path = resolve_project_trusted_write_path(&cwd);
    let global_path = default_config_path();

    let scope =
        match Select::new("Save to:", vec![TrustScope::Project, TrustScope::Global]).prompt() {
            Ok(v) => v,
            Err(e) => {
                println_labeled(
                    "clai",
                    &format!("could not read choice: {e}"),
                    Severity::Warn,
                );
                return Ok(());
            }
        };

    let target = match scope {
        TrustScope::Project => proj_path,
        TrustScope::Global => global_path,
    };

    let wrote = append_remember_run_program(&target, program)?;
    if wrote {
        println_labeled(
            "clai",
            &format!(
                "Wrote `{base}` to [interactive].remember_run_programs in {}.",
                target.display()
            ),
            Severity::Ok,
        );
    }
    if !trusted_list_contains(remembered, &base) {
        remembered.push(base);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn append_creates_minimal_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("clai.toml");
        assert!(append_trusted_program(&p, "chmod").expect("append"));
        let raw = std::fs::read_to_string(&p).expect("read");
        assert!(raw.contains("trusted_programs"));
        assert!(raw.contains("chmod"));
        assert!(raw.contains("config_version"));
    }

    #[test]
    fn append_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("clai.toml");
        assert!(append_trusted_program(&p, "/usr/bin/chmod").expect("append"));
        assert!(!append_trusted_program(&p, "chmod").expect("append2"));
        let raw = std::fs::read_to_string(&p).expect("read");
        assert_eq!(raw.matches("chmod").count(), 1, "{}", raw);
    }

    #[test]
    fn append_merges_existing_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&p).expect("file");
        f.write_all(b"config_version = 1\n[interactive]\nexecution = \"confirm\"\n")
            .expect("write");
        assert!(append_trusted_program(&p, "git").expect("append"));
        let raw = std::fs::read_to_string(&p).expect("read");
        assert!(raw.contains("execution"));
        assert!(raw.contains("git"));
    }

    #[test]
    fn resolve_prefers_existing_clai_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let clai = dir.path().join("clai.toml");
        std::fs::write(&clai, b"[policy]\n").unwrap();
        let dot = dir.path().join(".clai/config.toml");
        std::fs::create_dir_all(dot.parent().unwrap()).unwrap();
        std::fs::write(&dot, b"[policy]\n").unwrap();
        assert_eq!(resolve_project_trusted_write_path(dir.path()), clai);
    }

    #[test]
    fn append_remember_run_merges_interactive_section() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("clai.toml");
        std::fs::write(
            p.as_path(),
            b"config_version = 1\n[interactive]\nexecution = \"confirm\"\n",
        )
        .unwrap();
        assert!(append_remember_run_program(&p, "ls").expect("append"));
        let raw = std::fs::read_to_string(&p).expect("read");
        assert!(raw.contains("remember_run_programs"));
        assert!(raw.contains("ls"));
        assert!(raw.contains("execution"));
    }
}
