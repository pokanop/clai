//! Run [`CommandProposal`] subprocesses with piped capture or inherited stdio ([`StreamStrategy`]).
//! **Unix / direct:** `pre_exec` still calls `setpgid(0, 0)` for both capture and inherit (unchanged).
//! **Windows / direct:** job object + breakaway spawn for both strategies (unchanged).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use wait_timeout::ChildExt;

use crate::ask_exit::{clai_ask_process_exit_for_child, CLAI_ASK_TIMEOUT_EXIT};
use crate::config::{ExecutionConfig, ExecutionMode};
use crate::error::{AppError, Result};
use crate::schema::CommandProposal;
use crate::stream_strategy::StreamStrategy;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub status: Option<i32>,
    /// Value `clai` should use as the **process** exit code for `clai ask` after this run
    /// (child exit, signal mapping, or [`CLAI_ASK_TIMEOUT_EXIT`] for executor timeout).
    pub clai_ask_process_exit: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[cfg(windows)]
struct JobHandle(HANDLE);

#[cfg(windows)]
impl JobHandle {
    fn new() -> std::io::Result<Self> {
        unsafe {
            let job = CreateJobObjectW(None, None)?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )?;
            Ok(Self(job))
        }
    }

    fn assign_process(&self, child: &std::process::Child) -> std::io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        unsafe {
            AssignProcessToJobObject(self.0, HANDLE(child.as_raw_handle() as *mut _))?;
        }
        Ok(())
    }

    fn terminate(&self, exit_code: u32) -> std::io::Result<()> {
        unsafe { TerminateJobObject(self.0, exit_code)? };
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn effective_cwd(proposal: &CommandProposal) -> Result<PathBuf> {
    if let Some(c) = &proposal.cwd {
        return Ok(PathBuf::from(c));
    }
    std::env::current_dir().map_err(Into::into)
}

fn build_command(proposal: &CommandProposal, execution: &ExecutionConfig) -> Result<Command> {
    if proposal.needs_shell {
        return Err(AppError::Msg(
            "shell execution not enabled; use argv without needs_shell".into(),
        ));
    }

    match execution.mode {
        ExecutionMode::Direct => {
            let mut cmd = Command::new(&proposal.program);
            cmd.args(&proposal.args);
            if let Some(c) = &proposal.cwd {
                cmd.current_dir(Path::new(c));
            }
            Ok(cmd)
        }
        ExecutionMode::Docker => {
            let work = effective_cwd(proposal)?;
            let host_dir = dunce::canonicalize(&work)
                .map_err(|e| AppError::Msg(format!("docker cwd: {}", e)))?;
            let work_display = host_dir.display().to_string();

            let image = execution.docker_image.as_deref().unwrap_or("alpine:latest");

            let mut cmd = Command::new("docker");
            cmd.args(["run", "--rm", "-i"]);
            for a in &execution.docker_extra_args {
                cmd.arg(a);
            }
            cmd.arg("-v")
                .arg(format!("{}:{}", work_display, work_display))
                .arg("-w")
                .arg(&work_display)
                .arg(image)
                .arg(&proposal.program);
            cmd.args(&proposal.args);
            Ok(cmd)
        }
        ExecutionMode::Bwrap => {
            #[cfg(not(unix))]
            {
                let _ = proposal;
                let _ = execution;
                Err(AppError::Msg(
                    "execution.mode \"bwrap\" is only supported on Unix".into(),
                ))
            }
            #[cfg(unix)]
            {
                let work = effective_cwd(proposal)?;
                let host_dir = dunce::canonicalize(&work)
                    .map_err(|e| AppError::Msg(format!("bwrap cwd: {}", e)))?;
                let work_str = host_dir.to_string_lossy();

                let mut cmd = Command::new("bwrap");
                cmd.args([
                    "--unshare-pid",
                    "--die-with-parent",
                    "--dev-bind",
                    "/",
                    "/",
                    "--bind",
                ]);
                cmd.arg(host_dir.as_os_str());
                cmd.arg(host_dir.as_os_str());
                cmd.args(["--chdir", work_str.as_ref()]);
                for a in &execution.bwrap_extra_args {
                    cmd.arg(a);
                }
                cmd.arg("--");
                cmd.arg(&proposal.program);
                cmd.args(&proposal.args);
                Ok(cmd)
            }
        }
    }
}

/// Runs the proposal: either **piped capture** (null stdin, size-limited read of stdout/stderr)
/// or **inherited stdio** when `stream_strategy` is [`StreamStrategy::Inherit`] (direct host runs
/// only; Docker/bwrap use capture — see [`select_stream_strategy`](crate::stream_strategy::select_stream_strategy)).
///
/// For inherited stdio, child output is not captured into [`RunOutcome`]; the terminal shows it
/// directly. Timeout behavior matches the capture path (kill + `timed_out`).
pub fn run_proposal(
    proposal: &CommandProposal,
    timeout: std::time::Duration,
    max_capture_bytes: usize,
    execution: &ExecutionConfig,
    stream_strategy: StreamStrategy,
) -> Result<RunOutcome> {
    if stream_strategy == StreamStrategy::Inherit && execution.mode != ExecutionMode::Direct {
        return Err(AppError::Msg(
            "inherited stdio is only valid for execution.mode = \"direct\"".into(),
        ));
    }

    let mut cmd = build_command(proposal, execution)?;
    match stream_strategy {
        StreamStrategy::Capture => {
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }
        StreamStrategy::Inherit => {
            cmd.stdin(Stdio::inherit());
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        }
    }

    #[cfg(unix)]
    if execution.mode == ExecutionMode::Direct {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                let _ = libc::setpgid(0, 0);
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    let (mut child, win_job): (std::process::Child, Option<JobHandle>) =
        if execution.mode == ExecutionMode::Direct {
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
            cmd.creation_flags(CREATE_BREAKAWAY_FROM_JOB);
            let job = JobHandle::new().map_err(|e| AppError::Msg(format!("job object: {}", e)))?;
            let child = cmd.spawn()?;
            job.assign_process(&child)
                .map_err(|e| AppError::Msg(format!("assign job: {}", e)))?;
            (child, Some(job))
        } else {
            (cmd.spawn()?, None)
        };

    #[cfg(not(windows))]
    let mut child = cmd.spawn()?;

    let exit = match child.wait_timeout(timeout)? {
        Some(st) => st,
        None => {
            #[cfg(windows)]
            match &win_job {
                Some(j) => {
                    let _ = j.terminate(1);
                }
                None => {
                    let _ = child.kill();
                }
            }
            #[cfg(not(windows))]
            {
                let _ = child.kill();
            }
            let _ = child.wait();
            return Ok(RunOutcome {
                status: None,
                clai_ask_process_exit: CLAI_ASK_TIMEOUT_EXIT,
                stdout: String::new(),
                stderr: String::from("(killed: timeout)"),
                timed_out: true,
            });
        }
    };

    finish_child(child, exit, max_capture_bytes, stream_strategy)
}

fn finish_child(
    mut child: std::process::Child,
    exit: std::process::ExitStatus,
    max_capture_bytes: usize,
    stream_strategy: StreamStrategy,
) -> Result<RunOutcome> {
    if stream_strategy == StreamStrategy::Inherit {
        let clai_ask_process_exit = clai_ask_process_exit_for_child(&exit);
        return Ok(RunOutcome {
            status: exit.code(),
            clai_ask_process_exit,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
        });
    }

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
        clai_ask_process_exit: clai_ask_process_exit_for_child(&exit),
        stdout,
        stderr,
        timed_out: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExecutionConfig;
    use crate::schema::CommandProposal;
    use crate::stream_strategy::StreamStrategy;

    fn execution_direct() -> ExecutionConfig {
        ExecutionConfig {
            mode: ExecutionMode::Direct,
            ..Default::default()
        }
    }

    fn ok_proposal() -> CommandProposal {
        #[cfg(unix)]
        {
            CommandProposal {
                program: "true".to_string(),
                args: vec![],
                cwd: None,
                reason: None,
                needs_shell: false,
                confidence: None,
            }
        }
        #[cfg(windows)]
        {
            CommandProposal {
                program: "cmd".to_string(),
                args: vec!["/C".to_string(), "exit".to_string(), "0".to_string()],
                cwd: None,
                reason: None,
                needs_shell: false,
                confidence: None,
            }
        }
    }

    #[test]
    fn capture_trivial_succeeds() {
        let out = run_proposal(
            &ok_proposal(),
            std::time::Duration::from_secs(5),
            16 * 1024,
            &execution_direct(),
            StreamStrategy::Capture,
        )
        .expect("run");
        assert!(!out.timed_out);
        assert_eq!(out.status, Some(0), "expected success exit, got {:?}", out);
        assert_eq!(out.clai_ask_process_exit, 0);
    }

    /// Exit-code mapping in [`crate::ask_exit`] matches `RunOutcome::clai_ask_process_exit` for
    /// a failing trivial child (non-TTY capture path; task 1.8).
    #[test]
    fn capture_failing_child_preserves_status_and_clai_ask_process_exit() {
        let proposal = {
            #[cfg(unix)]
            {
                CommandProposal {
                    program: "sh".to_string(),
                    args: vec!["-c".to_string(), "exit 7".to_string()],
                    cwd: None,
                    reason: None,
                    needs_shell: false,
                    confidence: None,
                }
            }
            #[cfg(windows)]
            {
                CommandProposal {
                    program: "cmd".to_string(),
                    args: vec!["/C".to_string(), "exit".to_string(), "7".to_string()],
                    cwd: None,
                    reason: None,
                    needs_shell: false,
                    confidence: None,
                }
            }
        };
        let out = run_proposal(
            &proposal,
            std::time::Duration::from_secs(5),
            16 * 1024,
            &execution_direct(),
            StreamStrategy::Capture,
        )
        .expect("run");
        assert!(!out.timed_out);
        assert_eq!(out.status, Some(7));
        assert_eq!(out.clai_ask_process_exit, 7);
    }

    #[test]
    fn inherit_trivial_succeeds() {
        let out = run_proposal(
            &ok_proposal(),
            std::time::Duration::from_secs(5),
            16 * 1024,
            &execution_direct(),
            StreamStrategy::Inherit,
        )
        .expect("run");
        assert!(!out.timed_out);
        assert_eq!(out.status, Some(0), "expected success exit, got {:?}", out);
        assert_eq!(out.clai_ask_process_exit, 0);
        assert!(out.stdout.is_empty() && out.stderr.is_empty());
    }

    /// Short timeout; child sleeps long. Unix only so CI is deterministic without Windows sleep quirks.
    #[test]
    #[cfg(unix)]
    fn run_proposal_timeout_sets_process_exit_124() {
        use std::time::Duration;

        use crate::ask_exit::CLAI_ASK_TIMEOUT_EXIT;

        let p = CommandProposal {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "sleep 100".to_string()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        };
        let out = run_proposal(
            &p,
            Duration::from_millis(200),
            16 * 1024,
            &execution_direct(),
            StreamStrategy::Capture,
        )
        .expect("run");
        assert!(out.timed_out, "expected timeout, got {out:?}");
        assert_eq!(out.clai_ask_process_exit, CLAI_ASK_TIMEOUT_EXIT);
    }

    #[test]
    fn inherit_rejected_for_docker() {
        let e = run_proposal(
            &ok_proposal(),
            std::time::Duration::from_secs(5),
            16 * 1024,
            &ExecutionConfig {
                mode: ExecutionMode::Docker,
                ..Default::default()
            },
            StreamStrategy::Inherit,
        );
        assert!(e.is_err());
    }
}
