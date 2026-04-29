<!-- PRD: plans/interactive-command-history/prd.md -->
<!-- Tasks: plans/interactive-command-history/tasks.md -->
<!-- Closed: 2026-04-28 -->

# Retrospective: Interactive command history

> Shell-like Up/Down history and line editing at the main `clai>` prompt on TTY, with bounded in-memory storage, config, docs, and non-TTY fallback—delivered in one implementation push with all listed tasks completed.

## Summary

Interactive mode previously used a plain stdin reader, so arrow-key history and comfortable editing did not match shell/REPL expectations. The team shipped a TTY-gated path using **rustyline** (with explicit history append policy), a dedicated **InteractiveHistoryStore** (qualifying lines only, consecutive dedup, cap and character budget), configuration via **TOML and env**, and updates to **help** and **README**. Non-TTY sessions remain on the legacy reader with safe fallback when line editing cannot initialize. Five concrete tasks covering implementation, configuration, documentation, and quality gates were all completed; the task overview table in `tasks.md` still says “Total 8,” which does not match the five checklist items and should be corrected when editing historical artifacts is allowed.

## Metrics

| Metric | Value |
|--------|-------|
| Total tasks (top-level checkboxes) | 5 |
| Completed `[x]` | 5 (100%) |
| Skipped `[-]` | 0 |
| Blocked `[!]` | 0 |
| Not started `[ ]` | 0 |
| Effective completion rate | **100%** (5 / (5 − 0)) |
| PRD requirements covered (FR + US + NFR + QG) | **19 / 21 ~90%** (see interpretation) |
| Tasks without explicit PRD labels in `tasks.md` | 5 (all map logically to FR/US/NFR/QG; none list `FR-N` traceability fields) |
| PRD requirements never implemented | **0** for core FR/US scope; **verification gaps** for NFR-1 benchmark and SC-3 study (below) |

**Interpretation:** Completion is full for the recorded task list. Coverage uses the skill’s convention (FR, US, NFR, QG only = 21 items). **NFR-1** (p95 input latency ≤ 50 ms with documented test setup) is not reflected as a dedicated task or cited test command in `tasks.md`. **QG-5** (code review) is process, not a codebase deliverable. **Executive success criteria SC-1–SC-5** are not the same as the 21-label inventory; **SC-3** (moderated usability study, ≥80% first-try success) has no corresponding task—treat as out-of-band product validation or a documented gap.

**Phase breakdown**

| Phase | Completed | Total (non-skipped) | Rate |
|-------|-----------|---------------------|------|
| Phase 1: Core implementation | 4 | 4 | 100% |
| Phase 2: Verify | 1 | 1 | 100% |

## What Was Built

- TTY-only interactive line input with **Up/Down** history and standard line editing via **rustyline** (styled `clai>` prompt when supported).
- **InteractiveHistoryStore**: only non-empty model-qualifying submits; excludes session builtins; consecutive duplicate suppression; max entry count (default 1000, min 100) and **~4 MiB** character budget with oldest-first eviction.
- **RecordQualifyingLineOnDrop** so history is recorded after the line is consumed on the model path, including failed parses/execution—matching **OQ-1** resolution in `decisions.md`.
- **Configuration**: `[interactive].history_max_entries` and **`CLAI_INTERACTIVE__HISTORY_MAX_ENTRIES`** env, with parsing tests and documentation.
- **FR-8** behavior: non-TTY unchanged; rustyline init failure logs a warning and falls back to `read_line`; standalone store still applies policy when fallback is used on TTY.
- **SIGWINCH / resize**: retry loop until non-signal outcome (**D-4**).
- **Documentation** in session help and README (TTY vs pipe, builtins excluded, cap, privacy caveat).
- **Quality gates** run and noted complete: `cargo fmt`, `cargo test --no-default-features --locked`, `cargo clippy -D warnings`, `cargo build --locked`.
- **Unit tests** for history policy (per task notes; aligns with **NFR-4** intent).

## Scope Drift

### Additions (built beyond the PRD)

*No additions beyond PRD scope.* Optional crate choice (**rustyline**) and resize-loop behavior implement stated FRs; they are not separate product features.

### Deferrals (planned but deferred)

- **NG-3 / Phase 2 (PRD):** Cross-session persistence, **Ctrl+R**, and other polish remain explicitly **out of v1** per PRD—no scope creep attempted.
- **SC-3 (Executive success criterion):** Moderated usability check with three participants is **not** recorded as a task or outcome in `tasks.md`. If required for “done,” treat as **follow-up validation** (product / design owner); otherwise document waiver.
- **NFR-1:** Formal p95 latency measurement on reference hardware is **not** task-tracked; reliance is implicit (crate + manual smoke) unless a bench is added later.

### Blockers (planned but blocked)

*None.*

## Key Decisions

### D-1: `rustyline` for TTY input

**Context:** PRD needs arrow-key history and line editing; stdlib is insufficient.  
**Decision:** Add **rustyline** 18 with `default-features = false`; drive history via explicit `add_history_entry` for qualifying submits only.  
**Impact:** Future terminal work should assume this stack unless MSRV or portability forces re-evaluation.

See: `decisions.md` D-1

### D-2: Append history on Drop after model path

**Context:** **FR-2** / **FR-3** require builtins excluded and recording after consumption.  
**Decision:** **`RecordQualifyingLineOnDrop`** wraps the post-classification model/policy/execution path so all outcomes (including failures) record once, with **FR-4** dedup in the store.  
**Impact:** History semantics stay aligned with “consumed request”; avoid recording on early builtins.

See: `decisions.md` D-2 / **OQ-1**

### D-3: Plain stdin fallback with store on TTY

**Context:** **FR-8** when rustyline cannot initialize.  
**Decision:** Warn and fall back to `read_line` + prompt print; keep **`InteractiveHistoryStore`** on TTY so policy remains consistent even without recall UX.  
**Impact:** Tests and support should distinguish “no recall” from “no history policy.”

See: `decisions.md` D-3

### D-4: Handle resize signals

**Context:** Rustyline can return signal errors on **SIGWINCH**.  
**Decision:** Re-read in a loop until a non-resize outcome.  
**Impact:** Resize during prompt is expected to recover without crashing.

See: `decisions.md` D-4

## What Worked Well

- **Clear TTY vs non-TTY split** preserved scriptability and avoided the “always-on readline breaks pipes” failure mode from the PRD risk list.
- **Central history policy module** plus **Drop-based recording** kept **FR-2**–**FR-4** coherent without scattering rules next to every `continue`.
- **`decisions.md`** captured crate choice, append timing, fallback, and resize behavior—fast onboarding for the retro and for future maintainers.
- **Single-session push** with explicit “Quality gates” task mirrored **QG-1–QG-4** and kept merge criteria visible.

## What to Improve

- **`tasks.md` overview** says Total **8** but only **five** top-level tasks exist—fix the table when policy allows editing archived-bound files, or readers will distrust metrics.
- **Traceability fields:** Tasks do not cite **`FR-N` / `US-N`** labels; the next plan should add a Requirements column or bullets so coverage math is mechanical, not inferred.
- **NFR-1 and SC-3** were easy to lose because they were not decomposed into tasks; for similar PRDs, add explicit tasks (“perf smoke script,” “usability script” or explicit waiver).
- **`tasks.md` note** that it was created at implementation time without a prior **prd-to-tasks** artifact—earlier task generation would have caught the SC/NFR gaps earlier.

## Open Items

*All tasks are closed.* Carry-forward validation (if desired):

| Item | Status | Next step |
|------|--------|-----------|
| SC-3 moderated usability | Not in task list | Product: run study or record waiver |
| NFR-1 p95 verification | Not in task list | Add micro-bench or document “good enough” rationale |

## Future Opportunities

- **PRD Phase 2:** Optional disk persistence (behind flag, privacy-reviewed), **Ctrl+R** incremental search, session-specific history file.
- **Windows interactive parity:** **OQ-3** remains for maintainers if Windows becomes a first-class interactive target.
