//! Integration: `run_proposal` on the **direct** profile under typical **non-TTY** test-harness
//! conditions (no PTY, Phase 1). Asserts child exit code flows into `RunOutcome` and
//! `clai_ask_process_exit` (US-4, NFR-1, FR-3, SC-1, task 1.9).
//!
//! Deliberately lives in `tests/` (separate crate) as library-level “integration” per PRD.

use std::time::Duration;

use clai::config::{ExecutionConfig, ExecutionMode};
use clai::executor::run_proposal;
use clai::schema::CommandProposal;
use clai::stream_strategy::{
    select_stream_strategy, OutputIntent, StreamStrategy, UserTerminalContext,
};

const TIMEOUT: Duration = Duration::from_secs(15);
const CAP: usize = 64 * 1024;

/// Synthetic non-TTY context (CI and `cargo test` are not a terminal; no pty, task 1.9 / US-4).
fn ctx_non_tty() -> UserTerminalContext {
    UserTerminalContext {
        stdin_is_tty: false,
        stdout_is_tty: false,
        stderr_is_tty: false,
    }
}

fn execution_direct() -> ExecutionConfig {
    ExecutionConfig {
        mode: ExecutionMode::Direct,
        ..Default::default()
    }
}

fn success_proposal() -> CommandProposal {
    #[cfg(unix)]
    {
        CommandProposal {
            program: "true".to_string(),
            args: vec![],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
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
            script_body: None,
            script_extension: None,
        }
    }
}

fn false_proposal() -> CommandProposal {
    #[cfg(unix)]
    {
        CommandProposal {
            program: "false".to_string(),
            args: vec![],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        }
    }
    #[cfg(windows)]
    {
        CommandProposal {
            program: "cmd".to_string(),
            args: vec!["/C".to_string(), "exit".to_string(), "1".to_string()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        }
    }
}

fn sh_exit_n(n: i32) -> CommandProposal {
    #[cfg(unix)]
    {
        CommandProposal {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), format!("exit {n}")],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        }
    }
    #[cfg(windows)]
    {
        CommandProposal {
            program: "cmd".to_string(),
            args: vec!["/C".to_string(), "exit".to_string(), n.to_string()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        }
    }
}

#[test]
fn non_tty_capture_child_exit_0() {
    let out = run_proposal(
        &success_proposal(),
        TIMEOUT,
        CAP,
        &execution_direct(),
        StreamStrategy::Capture,
    )
    .expect("run");
    assert!(!out.timed_out);
    assert_eq!(out.status, Some(0));
    assert_eq!(out.clai_ask_process_exit, 0);
}

#[test]
fn non_tty_capture_child_exit_1() {
    let out = run_proposal(
        &false_proposal(),
        TIMEOUT,
        CAP,
        &execution_direct(),
        StreamStrategy::Capture,
    )
    .expect("run");
    assert!(!out.timed_out);
    assert_eq!(out.status, Some(1));
    assert_eq!(out.clai_ask_process_exit, 1);
}

#[test]
fn non_tty_capture_child_exit_42() {
    let out = run_proposal(
        &sh_exit_n(42),
        TIMEOUT,
        CAP,
        &execution_direct(),
        StreamStrategy::Capture,
    )
    .expect("run");
    assert!(!out.timed_out);
    assert_eq!(out.status, Some(42));
    assert_eq!(out.clai_ask_process_exit, 42);
}

/// **Inherit** path: I/O is not read back, but `ExitStatus` still drives `clai_ask_process_exit`.
#[test]
fn non_tty_inherit_child_exit_0() {
    let out = run_proposal(
        &success_proposal(),
        TIMEOUT,
        CAP,
        &execution_direct(),
        StreamStrategy::Inherit,
    )
    .expect("run");
    assert!(!out.timed_out);
    assert_eq!(out.status, Some(0));
    assert_eq!(out.clai_ask_process_exit, 0);
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

#[test]
fn non_tty_inherit_child_exit_19() {
    let out = run_proposal(
        &sh_exit_n(19),
        TIMEOUT,
        CAP,
        &execution_direct(),
        StreamStrategy::Inherit,
    )
    .expect("run");
    assert!(!out.timed_out);
    assert_eq!(out.status, Some(19));
    assert_eq!(out.clai_ask_process_exit, 19);
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

/// **Verbose** output intent (FR-1) always selects `Capture` on direct; should match an explicit
/// `StreamStrategy::Capture` for the same argv (task 1.9: path distinct from `Inherit`).
#[test]
fn verbose_resolves_to_capture_on_non_tty_and_propagates_exit() {
    let ctx = ctx_non_tty();
    let stream = select_stream_strategy(ExecutionMode::Direct, OutputIntent::Verbose, ctx, false);
    assert_eq!(
        stream,
        StreamStrategy::Capture,
        "verbose opt-in must use piped capture, not TTY-inherit, under non-TTY"
    );
    let out = run_proposal(&sh_exit_n(9), TIMEOUT, CAP, &execution_direct(), stream).expect("run");
    assert!(!out.timed_out);
    assert_eq!(out.status, Some(9));
    assert_eq!(out.clai_ask_process_exit, 9);

    let out2 = run_proposal(
        &sh_exit_n(9),
        TIMEOUT,
        CAP,
        &execution_direct(),
        StreamStrategy::Capture,
    )
    .expect("run2");
    assert_eq!(out2.clai_ask_process_exit, out.clai_ask_process_exit);
    assert_eq!(out2.status, out.status);
}
