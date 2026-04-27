//! FR-12: bare `clai` with no subcommand must not block when stdio is not a TTY.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn bare_clai_non_tty_exits_quickly_with_code_2() {
    let exe = env!("CARGO_BIN_EXE_clai");
    let start = Instant::now();
    let out = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .expect("spawn clai");
    let elapsed = start.elapsed();
    assert_eq!(
        out.code(),
        Some(2),
        "expected exit code 2 for non-TTY default invocation, got {out:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "expected quick exit, took {elapsed:?}"
    );
}

#[test]
fn clai_interactive_non_tty_also_exits_2() {
    let exe = env!("CARGO_BIN_EXE_clai");
    let out = Command::new(exe)
        .arg("interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .expect("spawn clai interactive");
    assert_eq!(out.code(), Some(2));
}
