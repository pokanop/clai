//! TTY line editing for the interactive session (FR-1, FR-8).

use crate::cli_output::styled_clai_prompt_ansi_fragment;
use crate::interactive_history::{sanitize_history_max_entries, InteractiveHistoryStore};
use crate::tty::{eprintln_labeled, println_labeled, Severity};
use rustyline::config::{CompletionType, Config};
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::Editor;
use std::io::{self, IsTerminal};

const CLAI_PROMPT_RAW: &str = "clai> ";

pub struct TtyInteractiveLineEditor {
    editor: Editor<(), DefaultHistory>,
    history_cap: usize,
    pub(crate) history: InteractiveHistoryStore,
    styled_prompt: Option<String>,
}

impl TtyInteractiveLineEditor {
    pub fn try_new(max_history_entries: usize) -> Option<Self> {
        let cap = sanitize_history_max_entries(max_history_entries);
        let config = match Config::builder().max_history_size(cap) {
            Ok(b) => match b.history_ignore_space(true).history_ignore_dups(true) {
                Ok(b2) => b2.completion_type(CompletionType::Circular).build(),
                Err(e) => {
                    eprintln_labeled(
                        "clai",
                        &format!(
                            "interactive config: {e}; using plain stdin (no Up/Down history)."
                        ),
                        Severity::Warn,
                    );
                    return None;
                }
            },
            Err(e) => {
                eprintln_labeled(
                    "clai",
                    &format!("interactive config: {e}; using plain stdin (no Up/Down history)."),
                    Severity::Warn,
                );
                return None;
            }
        };

        match Editor::<(), DefaultHistory>::with_config(config) {
            Ok(editor) => Some(Self {
                editor,
                history_cap: cap,
                history: InteractiveHistoryStore::new(cap),
                styled_prompt: styled_clai_prompt_ansi_fragment(),
            }),
            Err(e) => {
                eprintln_labeled(
                    "clai",
                    &format!(
                        "interactive line editor unavailable ({e}); using plain stdin (no Up/Down history)."
                    ),
                    Severity::Warn,
                );
                None
            }
        }
    }

    pub fn history_cap_entries(&self) -> usize {
        self.history_cap
    }

    /// Read one line; **EOF** is `Ok(None)`. Caller handles builtins and empties.
    pub fn read_line(&mut self) -> Result<Option<String>, io::Error> {
        loop {
            let read = if let Some(styled) = self.styled_prompt.as_deref() {
                self.editor.readline(&(CLAI_PROMPT_RAW, styled))
            } else {
                self.editor.readline(CLAI_PROMPT_RAW)
            };
            match read {
                Ok(s) => return Ok(Some(s)),
                Err(ReadlineError::Eof) => return Ok(None),
                Err(ReadlineError::Interrupted) => {
                    println!();
                    println_labeled(
                        "clai",
                        "(cancelled — empty line; type a request or exit)",
                        Severity::Info,
                    );
                    return Ok(Some(String::new()));
                }
                Err(ReadlineError::Signal(_)) => continue,
                Err(e) => return Err(io::Error::other(e)),
            }
        }
    }

    /// Record a qualifying model-request line after it was consumed (FR-3); updates readline history.
    pub fn record_qualifying_submit(&mut self, trimmed: &str) {
        if self.history.push_qualifying(trimmed) {
            let _ = self.editor.add_history_entry(trimmed);
        }
    }
}

/// After a non-builtin line is handled as a model request, append to session history on drop (FR-3).
pub(crate) struct RecordQualifyingLineOnDrop<'a> {
    editor: Option<&'a mut TtyInteractiveLineEditor>,
    /// When line editing disabled but stdin is still a TTY (`rustyline` unavailable), retain policy only (no recall).
    store_only: Option<&'a mut InteractiveHistoryStore>,
    trimmed: &'a str,
}

impl<'a> RecordQualifyingLineOnDrop<'a> {
    pub(crate) fn new(
        editor: Option<&'a mut TtyInteractiveLineEditor>,
        store_only: Option<&'a mut InteractiveHistoryStore>,
        trimmed: &'a str,
    ) -> Self {
        Self {
            editor,
            store_only,
            trimmed,
        }
    }
}

impl Drop for RecordQualifyingLineOnDrop<'_> {
    fn drop(&mut self) {
        if let Some(ed) = self.editor.take() {
            ed.record_qualifying_submit(self.trimmed);
        } else if let Some(h) = self.store_only.take() {
            let _ = h.push_qualifying(self.trimmed);
        }
    }
}

/// Both stdin and stdout are TTYs; same gating as the default interactive entrypoint (see `main`).
pub fn stdin_stdout_interactive_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}
