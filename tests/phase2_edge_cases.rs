//! Phase 2: capture-path limits, timeout, and lossy output (PRD risk / task 2.4–2.5).
//! Shell-based cases are Unix-only; CI is Ubuntu + macOS.

use clai::schema::CommandProposal;

#[cfg(unix)]
mod unix {
    use std::time::Duration;

    use clai::ask_exit::CLAI_ASK_TIMEOUT_EXIT;
    use clai::config::{ExecutionConfig, ExecutionMode};
    use clai::executor::run_proposal;
    use clai::stream_strategy::StreamStrategy;

    use super::CommandProposal;

    fn direct() -> ExecutionConfig {
        ExecutionConfig {
            mode: ExecutionMode::Direct,
            ..Default::default()
        }
    }

    /// Large single-stream write: first read is capped (see `finish_child` in `executor`).
    /// Total bytes must stay below the OS pipe capacity so the child can exit before `wait`
    /// returns; otherwise the child blocks on `write` and the parent deadlocks until timeout.
    #[test]
    fn capture_truncates_large_stdout() {
        let cap = 9 * 1024;
        let total = 12 * 1024;
        let p = CommandProposal {
            program: "python3".to_string(),
            args: vec![
                "-c".to_string(),
                format!("import sys; sys.stdout.buffer.write(b'A' * {total})"),
            ],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        };
        let out = run_proposal(
            &p,
            Duration::from_secs(30),
            cap,
            &direct(),
            StreamStrategy::Capture,
        )
        .expect("run");
        assert!(!out.timed_out);
        assert_eq!(out.stdout.len(), cap);
    }

    /// Same as `executor::run_proposal_timeout_sets_process_exit_124` but from the integration crate.
    #[test]
    fn capture_run_times_out() {
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
            Duration::from_millis(250),
            8 * 1024,
            &direct(),
            StreamStrategy::Capture,
        )
        .expect("run");
        assert!(out.timed_out, "expected timeout, got {out:?}");
        assert_eq!(out.clai_ask_process_exit, CLAI_ASK_TIMEOUT_EXIT);
    }

    /// Invalid UTF-8 is surfaced via `String::from_utf8_lossy` (U+FFFD). Verbose `ask` may print a
    /// one-line `stderr` note when replacement chars are present; default human capture still omits
    /// that note (see README).
    #[test]
    fn capture_stdout_non_utf8_is_lossy() {
        let p = CommandProposal {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "printf '\\xff\\xfe\\n'".to_string()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        };
        let out = run_proposal(
            &p,
            Duration::from_secs(10),
            4 * 1024,
            &direct(),
            StreamStrategy::Capture,
        )
        .expect("run");
        assert!(out.stdout.contains('\u{FFFD}') || out.stderr.contains('\u{FFFD}'));
    }
}

/// Policy gating is enforced in `cmd_ask` before `run_proposal`. Regression anchor for task 2.5
/// (same case as `policy::tests::blocks_rm_rf_root`).
#[test]
fn policy_still_blocks_obvious_destructive_proposal() {
    use clai::policy::PolicyEngine;
    use std::path::PathBuf;

    let eng = PolicyEngine::new(PathBuf::from("/tmp"), false, vec![]);
    let p = CommandProposal {
        program: "rm".into(),
        args: vec!["-rf".into(), "/".into()],
        cwd: None,
        reason: None,
        needs_shell: false,
        confidence: None,
    };
    let d = eng.evaluate(&p);
    assert!(d.blocked, "expected block for rm -rf /, got {d:?}");
}
