//! Process exit codes for the `clai` binary when `clai ask` runs a child process.
//!
//! # Semantics
//!
//! - If the child exits with a status code, [`ExitStatus::code`][`std::process::ExitStatus`]
//!   is used as the `clai` process exit code (including `0`).
//! - On **Unix**, if the process was stopped by a signal, `ExitStatus::code()` is `None` and this
//!   module maps to `128 + signal` (Bash’s convention) when the signal number is available.
//! - On **Windows** and other cases without a code or a Unix signal, the mapping returns `1`.
//! - If the run hits the executor’s **timeout** and the child is killed, see [`CLAI_ASK_TIMEOUT_EXIT`]
//!   (the same as GNU `timeout(1)` on many Linux systems).
//! - If the user **declines** the confirmation prompt, see [`CLAI_ASK_USER_DECLINED_EXIT`].
//! - If **dry-run** is in effect and no child is executed, see [`CLAI_ASK_DRY_RUN_EXIT`].
//! - If **policy** blocks or **`clai` errors** before a run, the process still exits with a
//!   non-zero code (see the error path in `main`, typically `1`).

/// `clai` process exit when the proposed command is killed for exceeding the `ask` run timeout.
/// Matches `timeout(1)` on GNU coreutils, so scripts can treat `124` as a timeout.
pub const CLAI_ASK_TIMEOUT_EXIT: i32 = 124;

/// User said no at the “run this command?” prompt; no child was run (FR-4, US-2).
pub const CLAI_ASK_USER_DECLINED_EXIT: i32 = 2;

/// Proposed command was not executed because `policy.dry_run_default` is set and `--yes` was
/// not passed (NFR-3, FR-4, US-2). Not the exit code of any child.
pub const CLAI_ASK_DRY_RUN_EXIT: i32 = 3;

/// Maps a child’s [`std::process::ExitStatus`] to the `clai` process exit code to use for `ask`
/// (after a completed wait — not timeout).
#[must_use]
pub fn clai_ask_process_exit_for_child(exit: &std::process::ExitStatus) -> i32 {
    if let Some(code) = exit.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = exit.signal() {
            return 128 + sig;
        }
    }
    1
}

/// Runs a short-lived process that exits with `n` and returns its `ExitStatus` (tests).
#[cfg(test)]
fn exit_status_code_n(n: i32) -> std::io::Result<std::process::ExitStatus> {
    use std::process::Command;
    use std::process::Stdio;

    use wait_timeout::ChildExt;

    let mut c = {
        #[cfg(unix)]
        {
            let mut c = Command::new("sh");
            c.args(["-c", &format!("exit {n}")]);
            c
        }
        #[cfg(windows)]
        {
            let mut c = Command::new("cmd");
            c.args(["/C", "exit", &n.to_string()]);
            c
        }
    };
    c.stdin(Stdio::null());
    c.stdout(Stdio::null());
    c.stderr(Stdio::null());
    let mut ch = c.spawn()?;
    let t = std::time::Duration::from_secs(15);
    match ch.wait_timeout(t)? {
        Some(st) => Ok(st),
        None => {
            let _ = ch.kill();
            let _ = ch.wait();
            Err(std::io::Error::other("exit_status_code_n: wait timed out"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_zero_and_nonzero() {
        let s0 = exit_status_code_n(0).expect("helper");
        assert_eq!(clai_ask_process_exit_for_child(&s0), 0);
        let s1 = exit_status_code_n(1).expect("helper");
        assert_eq!(clai_ask_process_exit_for_child(&s1), 1);
        let s42 = exit_status_code_n(42).expect("helper");
        assert_eq!(clai_ask_process_exit_for_child(&s42), 42);
    }

    #[test]
    fn no_run_exit_codes_are_nonzero_and_distinct() {
        assert_ne!(CLAI_ASK_USER_DECLINED_EXIT, 0);
        assert_ne!(CLAI_ASK_DRY_RUN_EXIT, 0);
        assert_ne!(CLAI_ASK_TIMEOUT_EXIT, 0);
        assert_ne!(CLAI_ASK_USER_DECLINED_EXIT, CLAI_ASK_DRY_RUN_EXIT);
    }

    /// Subprocess is terminated by a signal: `clai_ask_process_exit_for_child` uses `128 + signal`
    /// on Unix (deterministic, no PTY).
    #[test]
    #[cfg(unix)]
    fn maps_unix_signal_termination() {
        use std::process::{Command, Stdio};

        let st = Command::new("sh")
            .arg("-c")
            .arg("kill -TERM $$")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn");
        // SIGTERM = 15 on POSIX; map matches Bash-style 128+signal
        let mapped = clai_ask_process_exit_for_child(&st);
        assert_eq!(
            mapped,
            128 + 15,
            "expected 128+SIGTERM, got {mapped} st={st:?}"
        );
    }
}
