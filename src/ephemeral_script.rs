//! Managed ephemeral script files (FR-5, FR-7, NFR-3, NFR-4).

use std::io::Write;
use std::path::Path;

use crate::config::ToolingConfig;
use crate::error::{AppError, Result};
use crate::schema::CommandProposal;

/// After materialization, the executable proposal and the temp directory (delete on drop).
pub struct PreparedCommand {
    pub proposal: CommandProposal,
    pub temp: Option<tempfile::TempDir>,
    /// Absolute path to the script file when materialized.
    pub script_path: Option<std::path::PathBuf>,
}

/// Strip script fields and append the temp script path to argv when `script_body` is set.
pub fn prepare_command_proposal(
    mut proposal: CommandProposal,
    tooling: &ToolingConfig,
) -> Result<PreparedCommand> {
    proposal.normalize_empty_script_fields();

    let body = match &proposal.script_body {
        Some(b) => b.clone(),
        None => {
            return Ok(PreparedCommand {
                proposal,
                temp: None,
                script_path: None,
            });
        }
    };

    if !tooling.ephemeral_scripts {
        return Err(AppError::Msg(
            "model proposed script_body but [tooling].ephemeral_scripts is false — enable it in config or use interpreter -c / inline argv instead".into(),
        ));
    }

    if proposal.needs_shell {
        return Err(AppError::Msg(
            "script_body cannot be combined with needs_shell — use argv to the interpreter only"
                .into(),
        ));
    }

    let dir = tempfile::Builder::new()
        .prefix("clai-script-")
        .tempdir()
        .map_err(|e| AppError::Msg(format!("ephemeral script temp dir: {e}")))?;

    let ext = normalized_extension(proposal.script_extension.as_deref(), &proposal.program);
    let file_path = dir.path().join(format!("script.{ext}"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&file_path)
            .map_err(|e| AppError::Msg(format!("ephemeral script file: {e}")))?;
        f.write_all(body.as_bytes())
            .map_err(|e| AppError::Msg(format!("ephemeral script write: {e}")))?;
    }
    #[cfg(not(unix))]
    {
        let mut f = std::fs::File::create(&file_path)
            .map_err(|e| AppError::Msg(format!("ephemeral script file: {e}")))?;
        f.write_all(body.as_bytes())
            .map_err(|e| AppError::Msg(format!("ephemeral script write: {e}")))?;
    }

    let path_str = file_path
        .to_str()
        .ok_or_else(|| AppError::Msg("ephemeral script path is not valid UTF-8".into()))?
        .to_string();

    let mut out = proposal;
    out.script_body = None;
    out.script_extension = None;
    let mut args = out.args.clone();
    args.push(path_str.clone());
    out.args = args;

    Ok(PreparedCommand {
        proposal: out,
        temp: Some(dir),
        script_path: Some(file_path),
    })
}

fn normalized_extension(ext: Option<&str>, program: &str) -> String {
    if let Some(e) = ext {
        let t = e.trim().trim_start_matches('.');
        if !t.is_empty() {
            return t.to_string();
        }
    }
    default_extension_for_program(program)
}

fn default_extension_for_program(program: &str) -> String {
    let lower = program.to_lowercase();
    let base = Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
        .to_lowercase();
    if base.contains("python") || lower.contains("python") {
        return "py".into();
    }
    if base.contains("node") || base == "node" {
        return "js".into();
    }
    if base.contains("ruby") {
        return "rb".into();
    }
    if base.contains("perl") {
        return "pl".into();
    }
    if base.contains("php") {
        return "php".into();
    }
    "txt".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ToolingConfig;

    #[test]
    fn materializes_and_strips_body() {
        let mut tooling = ToolingConfig::default();
        tooling.ephemeral_scripts = true;

        let p = CommandProposal {
            program: "python3".into(),
            args: vec!["-u".into()],
            cwd: None,
            reason: Some("test".into()),
            needs_shell: false,
            confidence: None,
            script_body: Some("print(1)".into()),
            script_extension: None,
        };

        let prep = prepare_command_proposal(p, &tooling).expect("prep");
        assert!(prep.proposal.script_body.is_none());
        assert_eq!(prep.proposal.args.len(), 2);
        assert_eq!(prep.proposal.args[0], "-u");
        assert!(prep.proposal.args[1].ends_with(".py"));
        assert!(prep.script_path.as_ref().unwrap().is_file());
        drop(prep);
    }

    #[test]
    fn rejects_body_when_disabled() {
        let tooling = ToolingConfig::default();
        let p = CommandProposal {
            program: "python3".into(),
            args: vec![],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: Some("x".into()),
            script_extension: None,
        };
        assert!(prepare_command_proposal(p, &tooling).is_err());
    }

    #[test]
    fn empty_script_body_is_no_script() {
        let mut tooling = ToolingConfig::default();
        tooling.ephemeral_scripts = true;

        let p = CommandProposal {
            program: "wc".into(),
            args: vec!["-l".into()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: Some(String::new()),
            script_extension: Some(String::new()),
        };
        let prep = prepare_command_proposal(p, &tooling).expect("prep");
        assert!(prep.temp.is_none());
        assert!(prep.script_path.is_none());
        assert!(prep.proposal.script_body.is_none());
        assert!(prep.proposal.script_extension.is_none());
        assert_eq!(prep.proposal.args, vec!["-l"]);
    }
}
