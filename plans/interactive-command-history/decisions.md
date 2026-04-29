# Decisions — interactive command history

## D-1: `rustyline` for TTY input

**Context:** PRD requires Up/Down history and line editing on the main prompt; stdlib cannot provide this.

**Decision:** Add `rustyline` 18 (`default-features = false`) with in-memory `DefaultHistory`; append entries only after a qualifying submit via explicit `add_history_entry` (no reliance on auto-add for every keystroke).

**Alternatives:** Hand-rolled ANSI (rejected per PRD); reedline (heavier NU integration).

---

## D-2: When to append history

**Context:** FR-2 / FR-3 require builtins excluded and recording after the line was consumed as a model request.

**Decision:** Wrap the entire model/policy/execution path after builtin classification with `RecordQualifyingLineOnDrop` (Drop at end of iteration): every `continue` from parse errors, policy block, skips, execution errors, or success records the trimmed line unless FR-4 dedup rejects it within `InteractiveHistoryStore`.

**Note (OQ-1):** Failed parses still record — user may edit and retry the same wording.

---

## D-3: Plain stdin fallback on TTY

**Context:** Rustyline can fail to initialize (FR-8).

**Decision:** Warn and fall back to `stdin.read_line` + `print_clai_prompt`; if stdin/stdout remain TTY, keep a standalone `InteractiveHistoryStore` so policy/eviction semantics still apply even without recall UX.

---

## D-4: SIGWINCH / resize

**Context:** Rustyline 18 emits `ReadlineError::Signal` on terminal resize.

**Decision:** Re-read line in a loop until a non-resize outcome (documented implicitly via prompt behavior matching common readline crates).
