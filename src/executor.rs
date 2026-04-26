use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use wait_timeout::ChildExt;

use crate::error::{AppError, Result};
use crate::schema::CommandProposal;

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub fn run_proposal(
    proposal: &CommandProposal,
    timeout: std::time::Duration,
    max_capture_bytes: usize,
) -> Result<RunOutcome> {
    if proposal.needs_shell {
        return Err(AppError::Msg(
            "shell execution not enabled; use argv without needs_shell".into(),
        ));
    }

    let mut cmd = Command::new(&proposal.program);
    cmd.args(&proposal.args);
    if let Some(c) = &proposal.cwd {
        cmd.current_dir(Path::new(c));
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                let _ = libc::setpgid(0, 0);
                Ok(())
            });
        }
    }

    let mut child = cmd.spawn()?;

    let exit = match child.wait_timeout(timeout)? {
        Some(st) => st,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(RunOutcome {
                status: None,
                stdout: String::new(),
                stderr: String::from("(killed: timeout)"),
                timed_out: true,
            });
        }
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let mut buf = vec![0u8; max_capture_bytes];
        let n = out.read(&mut buf).unwrap_or(0);
        stdout.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    if let Some(mut err) = child.stderr.take() {
        let mut buf = vec![0u8; max_capture_bytes];
        let n = err.read(&mut buf).unwrap_or(0);
        stderr.push_str(&String::from_utf8_lossy(&buf[..n]));
    }

    Ok(RunOutcome {
        status: exit.code(),
        stdout,
        stderr,
        timed_out: false,
    })
}
