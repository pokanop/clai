//! Stream connection strategy for proposal execution: inherit the user’s terminal
//! vs piped capture. Pure decision logic only — no policy or argv construction.
//!
//! See PRD: `ExecutionMode`, output intent (human default vs verbose), and TTY
//! attachment per [`UserTerminalContext`].

use std::io::IsTerminal;

use crate::config::ExecutionMode;

/// Whether the current process’s standard streams are attached to a user
/// terminal, as used to decide the direct + human “shell-native” path (FR-2).
///
/// Callers typically fill this with [`IsTerminal::is_terminal`] on each handle.
/// The PRD’s Phase 1 matrix treats “non-TTY” (e.g. CI) as: not suitable for
/// inheritance — use capture instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserTerminalContext {
    pub stdin_is_tty: bool,
    pub stdout_is_tty: bool,
    pub stderr_is_tty: bool,
}

impl UserTerminalContext {
    /// `true` when stdin, stdout, and stderr are all TTYs (interactive terminal).
    #[must_use]
    pub fn all_streams_tty(&self) -> bool {
        self.stdin_is_tty && self.stdout_is_tty && self.stderr_is_tty
    }
}

/// Human-default vs verbose / machine-oriented presentation (FR-1, US-3).
/// Verbose always forces piped capture regardless of TTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputIntent {
    /// Default human terminal UX; may inherit on direct + TTY.
    #[default]
    Human,
    /// Opt-in verbose / structured diagnostics; always capture.
    Verbose,
}

/// How the executor connects the child’s standard streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStrategy {
    /// Inherit stdin/stdout/stderr from this process (terminal-connected I/O).
    Inherit,
    /// Piped capture with size limits (non-direct, verbose, or non-TTY).
    Capture,
}

/// Returns whether the child should use inherited stdio vs piped capture.
///
/// - **Non-direct** profiles ([`ExecutionMode::Docker`], [`ExecutionMode::Bwrap`]) are
///   always capture-first in Phase 1 (FR-6, PRD §9).
/// - **Verbose** output intent always selects capture.
/// - If **`force_capture`** is set for [`ExecutionMode::Direct`], returns capture so operators
///   get size-limited piped I/O on a TTY (Phase 2) without changing policy.
/// - **Direct** + **human** selects [`StreamStrategy::Inherit`] only when
///   [`UserTerminalContext::all_streams_tty`] is true; otherwise capture (e.g. CI).
#[must_use]
pub fn select_stream_strategy(
    mode: ExecutionMode,
    output_intent: OutputIntent,
    tty: UserTerminalContext,
    force_capture: bool,
) -> StreamStrategy {
    if output_intent == OutputIntent::Verbose {
        return StreamStrategy::Capture;
    }
    if force_capture && mode == ExecutionMode::Direct {
        return StreamStrategy::Capture;
    }
    match mode {
        ExecutionMode::Docker | ExecutionMode::Bwrap => StreamStrategy::Capture,
        ExecutionMode::Direct => {
            if tty.all_streams_tty() {
                StreamStrategy::Inherit
            } else {
                StreamStrategy::Capture
            }
        }
    }
}

/// Builds a [`UserTerminalContext`] from the current process stdio (convenience for CLI).
#[must_use]
pub fn current_user_terminal_context() -> UserTerminalContext {
    UserTerminalContext {
        stdin_is_tty: std::io::stdin().is_terminal(),
        stdout_is_tty: std::io::stdout().is_terminal(),
        stderr_is_tty: std::io::stderr().is_terminal(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(stdin: bool, stdout: bool, stderr: bool) -> UserTerminalContext {
        UserTerminalContext {
            stdin_is_tty: stdin,
            stdout_is_tty: stdout,
            stderr_is_tty: stderr,
        }
    }

    #[test]
    fn direct_human_non_tty_is_capture() {
        assert_eq!(
            select_stream_strategy(
                ExecutionMode::Direct,
                OutputIntent::Human,
                ctx(false, false, false),
                false
            ),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn direct_human_all_tty_is_inherit() {
        assert_eq!(
            select_stream_strategy(
                ExecutionMode::Direct,
                OutputIntent::Human,
                ctx(true, true, true),
                false
            ),
            StreamStrategy::Inherit
        );
    }

    #[test]
    fn direct_human_all_tty_force_capture_overrides_inherit() {
        assert_eq!(
            select_stream_strategy(
                ExecutionMode::Direct,
                OutputIntent::Human,
                ctx(true, true, true),
                true
            ),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn direct_human_partial_tty_is_capture() {
        assert_eq!(
            select_stream_strategy(
                ExecutionMode::Direct,
                OutputIntent::Human,
                ctx(false, true, true),
                false
            ),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn verbose_direct_all_tty_still_capture() {
        assert_eq!(
            select_stream_strategy(
                ExecutionMode::Direct,
                OutputIntent::Verbose,
                ctx(true, true, true),
                false
            ),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn force_capture_ignored_for_verbose_on_direct() {
        assert_eq!(
            select_stream_strategy(
                ExecutionMode::Direct,
                OutputIntent::Verbose,
                ctx(true, true, true),
                true
            ),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn docker_always_capture() {
        let tty = ctx(true, true, true);
        assert_eq!(
            select_stream_strategy(ExecutionMode::Docker, OutputIntent::Human, tty, true),
            StreamStrategy::Capture
        );
        assert_eq!(
            select_stream_strategy(ExecutionMode::Docker, OutputIntent::Verbose, tty, false),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn bwrap_always_capture() {
        let tty = ctx(true, true, true);
        assert_eq!(
            select_stream_strategy(ExecutionMode::Bwrap, OutputIntent::Human, tty, true),
            StreamStrategy::Capture
        );
        assert_eq!(
            select_stream_strategy(ExecutionMode::Bwrap, OutputIntent::Verbose, tty, false),
            StreamStrategy::Capture
        );
    }

    #[test]
    fn all_streams_tty_helper_matches_selection_matrix() {
        assert!(!UserTerminalContext {
            stdin_is_tty: false,
            stdout_is_tty: true,
            stderr_is_tty: true,
        }
        .all_streams_tty());
        assert!(UserTerminalContext {
            stdin_is_tty: true,
            stdout_is_tty: true,
            stderr_is_tty: true,
        }
        .all_streams_tty());
    }

    #[test]
    fn default_output_intent_is_human() {
        assert_eq!(OutputIntent::default(), OutputIntent::Human);
    }
}
