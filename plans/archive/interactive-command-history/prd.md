# PRD: Interactive command history

## 1. Executive Summary

**Problem Statement:** In interactive mode, users type natural-language requests at the `clai>` prompt. Input is read with a plain stdin reader, so line editing and recall behave like a dumb pipe: arrow keys do not navigate prior requests, and repeating or tweaking a long prompt is tedious. This breaks expectations set by shells, REPLs, and most CLI tools.

**Proposed Solution:** Provide shell-like command history for the main interactive prompt so users can recall and edit past **requests** with minimal friction—primarily **Up** / **Down** to move through history, with behavior consistent with common terminal conventions. Scope is the primary session input loop only (not secondary `inquire` confirmation prompts).

**Success Criteria:**

1. **SC-1:** On an interactive TTY session, after the user has submitted at least one non-empty request that was processed as a model request (not merely `help` / `exit` / `reload`), pressing **Up** at an empty or freshly focused prompt recalls the most recent such entry within **300 ms** of key release (no perceptible lag for typical history sizes).
2. **SC-2:** At least **100** distinct prior requests are retainable in a single session without incorrect ordering when cycling **Up** then **Down** (newest-to-oldest then back toward present).
3. **SC-3:** In a five-minute moderated usability check (3 participants familiar with shells), **≥80%** successfully recall and resubmit a prior line using **Up** on the first try after a one-sentence hint (“like your shell history”).
4. **SC-4:** Non-TTY stdin (piped or redirected input) continues to behave as today: no hang, no panic, and documented or logged degradation if line editing is unavailable.
5. **SC-5:** All repository quality gates in Section 7 pass on CI-equivalent commands with **zero** new warnings at the CI warning level.

## 2. Goals and Non-Goals

### Goals

1. **G-1:** Shell-like history navigation at the main interactive prompt for past user requests.
2. **G-2:** Intuitive ordering: **Up** moves to older entries, **Down** moves to newer entries; the editing buffer reflects the selected history entry.
3. **G-3:** Only **substantive** request lines are stored (non-empty after trim); duplicates policy is defined in requirements (see **FR-5**).
4. **G-4:** Works on **macOS** and **Linux** interactive terminals; Windows behavior follows whatever the project already supports for interactive sessions (if unsupported, degrade per **FR-8**).
5. **G-5:** No regression to existing session semantics: builtins, EOF, Ctrl+C messaging, and execution modes remain unchanged aside from input acquisition.

### Non-Goals

- **NG-1:** We will not add history navigation to nested prompts (e.g., `inquire` confirm dialogs) in this initiative.
- **NG-2:** We will not implement cross-device or account-synced history cloud storage.
- **NG-3:** We will not require persistent history across process restarts in v1 (optional follow-up only if justified).
- **NG-4:** We will not implement full **readline** feature parity (macros, incremental search, vim/emacs modes) unless needed to meet **FR-1–FR-3**; incremental reverse search (**Ctrl+R**) is explicitly out of scope for the MVP phase unless listed as a later phase.
- **NG-5:** We will not log or transmit history contents to telemetry; history remains local to the process (and optional future persistence path on disk must be user-controlled and documented).

### Constraints

- **C-1:** Implementation must stay aligned with the existing **Rust / Cargo** codebase and **MIT OR Apache-2.0** licensing.
- **C-2:** CI today runs **`cargo fmt --check`**, **`cargo test --no-default-features --locked`**, **`cargo clippy --no-default-features --locked -D warnings`**, and **`cargo build --locked`**; changes must not weaken these gates.
- **C-3:** New dependencies (if any) must be justified against crates already in use and maintenance risk (see Section 4).
- **C-4:** Security: history must not be written to world-readable locations by default if disk persistence is added later; secrets pasted into the prompt are user-controlled—document risk in help/README.

### Scope Check

Single cohesive initiative (main prompt history). Functional requirement count stays within one subsystem (interactive session input).

## 3. User Stories and Requirements

### User Personas

- **P-1 — Interactive CLI user:** Runs `clai` in a terminal, types natural-language commands repeatedly; expects shell-like editing and recall.
- **P-2 — Power user / integrator:** May script or pipe stdin; needs predictable non-interactive behavior.

### User Stories

- **US-1:** As **P-1**, I want to press **Up** at the `clai>` prompt to recall my previous request so that I can rerun or edit it without retyping.  
  - **Acceptance Criteria:** Given at least one stored history entry, **Up** at the prompt loads the newest entry into the editable line; repeated **Up** moves to older entries until the oldest is reached. **Priority:** P0  
- **US-2:** As **P-1**, I want to press **Down** while browsing history to return to more recent entries so that I can recover from overscrolling.  
  - **Acceptance Criteria:** After moving up in history, **Down** moves forward; from the “present” state, **Down** clears to an empty new line (or equivalent documented behavior). **Priority:** P0  
- **US-3:** As **P-1**, I want basic line editing (cursor left/right, backspace/delete, Home/End if available) on recalled lines so that I can fix typos before submitting.  
  - **Acceptance Criteria:** On a TTY, a recalled line can be edited before **Enter**; behavior matches platform norms for the chosen input layer. **Priority:** P0  
- **US-4:** As **P-2**, I want non-TTY stdin to keep working so that automation does not break.  
  - **Acceptance Criteria:** Piped input sessions do not require a TTY-capable editor; behavior matches **FR-8**. **Priority:** P0  

### Functional Requirements

- **FR-1:** The system must use a line-input path for the main interactive loop when stdin is a TTY that supports **Up** / **Down** history navigation for stored entries.
- **FR-2:** The system must record a history entry only for lines that are not empty after trim and that are dispatched as user model requests (i.e., not classified as session builtins `exit`, `quit`, `help`, `?`, or `reload`).
- **FR-3:** The system must append a new history record when the user submits a qualifying line (**FR-2**) after any successful or failed model/policy/execution attempt that consumed that line as the user request (the line is “submitted” once **Enter** is pressed with that content).
- **FR-4:** The system must not duplicate consecutive identical qualifying lines in history (same string as the last stored entry ⇒ omit new append).
- **FR-5:** The system must cap in-memory history to a configurable maximum count (default **1000**, minimum **100**) and drop **oldest** entries when exceeding the cap; configuration surface may be env, config file, or both—implementation decides, but default and bounds must be documented.
- **FR-6:** The system must preserve prompt labeling (`clai>` and styling) consistent with current UX when using enhanced line input (no duplicate prompts on the same line).
- **FR-7:** The system must document history behavior in session `help` output and in README (TTY vs non-TTY, builtins excluded, cap).
- **FR-8:** When stdin is not a TTY or line editing cannot be initialized, the system must fall back to the existing line-based stdin reader without panicking; history navigation is unavailable in that mode.

### Non-Functional Requirements

- **NFR-1:** History navigation on a TTY with **≤1000** stored entries must not introduce input latency exceeding **50 ms** p95 between key event and rendered buffer update on reference dev hardware (developer laptop class; document test setup in test plan).
- **NFR-2:** Memory use for history must stay bounded by the configured cap and reasonable string sizes; total retained characters must not exceed **4 MiB** without eviction (evict oldest entries until under cap).
- **NFR-3:** The feature must not introduce new **clippy** warnings at `-D warnings` in CI configurations.
- **NFR-4:** Automated tests must cover history policy (**FR-2**, **FR-4**, cap behavior **FR-5**) at unit level; TTY integration may use a crate-supported test harness or conditional tests documented if OS-specific.

## 4. Solution Design

### Approach

Replace the “print prompt + `read_line`” pattern for **TTY** stdin with a single line editor that maintains an in-session history list conforming to **FR-2–FR-5**. Non-TTY sessions keep the current simple reader (**FR-8**). This aligns user expectations with shells and REPLs while preserving existing session control flow after a line is submitted.

### Key Design Decisions

| Decision | Context | Options Considered | Rationale | Trade-offs |
|----------|---------|-------------------|-----------|------------|
| TTY-only enhanced input | Piped stdin must still work | Always-on curses/readline (breaks pipes); TTY-gated | Matches **FR-8** and **US-4** | Two code paths to test |
| Exclude builtins from history | `help` spam would clutter history | Record everything | Cleaner history aligned with “requests” | Users cannot recall `help` text (acceptable) |
| Consecutive dedup | Repeated submits common | Keep duplicates | Less noise | Cannot intentionally store same line twice in a row |
| In-memory first | Simpler, no privacy surprises | Immediate disk persistence | **NG-3** defers persistence | History lost on exit until follow-up |

### Architecture Overview

- **Input layer:** TTY path obtains a full line string plus standard editing semantics; non-TTY path unchanged.
- **History store:** Bounded list owned by the session loop (or small dedicated module), fed only by qualifying submits.
- **Integration:** Existing trimming, builtin classification, and model invocation remain downstream of “line received.”

**New dependencies:** Any crate that provides line editing + history must be evaluated against **inquire** (already used for confirms) and stdlib; justification: stdlib does not offer arrow-key history. Prefer a maintained, widely used crate with explicit license compatibility. If no crate is added, the design must still satisfy **FR-1** (e.g., platform-specific APIs—only if complexity is lower than a vetted dependency).

### Modular Design Principles

- Isolate “read interactive line with optional history” behind a small interface so tests can inject fake inputs and simulate history mutations without spinning up a model.
- Keep policy of what qualifies for history next to builtin classification to avoid drift.

### Security Considerations

- History may contain sensitive tokens or paths; **NG-5** applies. If future disk persistence is added, default path must be user-owned (e.g., under XDG state dir / macOS Application Support), permissions restrictive, and documented. No automatic upload.

## 5. Alternatives Considered

- **Alternative:** Manual ANSI escape parsing on top of `read_line`.  
  - **Pros:** No new dependency.  
  - **Cons:** Fragile across terminals; high maintenance.  
  - **Verdict:** Rejected.

- **Alternative:** Record **all** lines including builtins.  
  - **Pros:** Simpler rules.  
  - **Cons:** Pollutes history; conflicts with “past requests” mental model.  
  - **Verdict:** Rejected.

- **Alternative:** Persist history to disk in v1.  
  - **Pros:** Cross-session recall.  
  - **Cons:** Privacy, path conventions, corruption handling; expands scope.  
  - **Verdict:** Deferred (**NG-3**).

## 6. Implementation Plan

### Phased Rollout

- **Phase 1 (MVP):** TTY-gated line editor with in-memory history, **Up**/**Down**, **FR-2–FR-8**, help/README updates, unit tests for history policy and cap.
- **Phase 2 (Polish):** Optional persistence behind a config flag (if product asks), incremental search (**Ctrl+R**), or session-specific history file—only after Phase 1 ships and privacy review.

### Tech Stack Alignment

Rust workspace, existing modules (`session`, `cli_output`, `interactive_mode`). Use `IsTerminal` (already referenced in codebase patterns) for gating.

### Migration and Compatibility

No data migration. Behavior change is user-visible improvement on TTY only; scripts using pipes unaffected.

## 7. Testing Strategy

### Testing Levels

- **Unit:** History eligibility (builtins excluded), dedup, cap eviction, character budget (**NFR-2**).
- **Integration:** Session loop receives lines from a fake input provider; ensures builtins do not mutate history.
- **E2E / Manual:** Developer checklist on macOS and Linux TTY: **Up**/**Down**, edit recalled line, pipe stdin still works.

### Validation Approach

Follow existing Rust test style (`#[cfg(test)]` modules). Add tests alongside `session` or a new small history module.

### Quality Gates

All of the following must pass before merge:

- **QG-1:** `cargo fmt --check` — Formatting matches project standards.
- **QG-2:** `cargo test --no-default-features --locked` — All tests pass with locked deps.
- **QG-3:** `cargo clippy --no-default-features --locked -- -D warnings` — No clippy warnings.
- **QG-4:** `cargo build --locked` — Full default-feature build succeeds (as in CI `build-full`).
- **QG-5:** Code review completed — At least one reviewer approves history privacy and TTY fallback behavior.

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Dependency maintenance or MSRV issues | Med | Med | Prefer widely used crate; pin versions; run `cargo deny` or audit if project adopts it |
| Terminal compatibility quirks | Med | Med | Test macOS + Linux CI/manual; document unsupported terminals; clear fallback (**FR-8**) |
| History retains secrets | Med | High | README warning; future persistence opt-in only |
| Scope creep (Ctrl+R, persistence) | Med | Med | **NG-3**, **NG-4**; Phase 2 only by explicit decision |
| Dual code path bugs | Low | Med | Unit tests + explicit non-TTY smoke test |

## 9. Open Questions

1. **OQ-1:** Should **failed** model parses still add the line to history? **Proposed default:** Yes—user may want to fix and resubmit the same wording; aligns **FR-3** with “consumed as user request.” **Owner:** Product / implementer. **Impact:** Low—document choice in help.
2. **OQ-2:** Exact config key namespace for history cap (e.g., `CLAI_INTERACTIVE__HISTORY_MAX`) vs TOML only? **Proposed default:** Match existing `CLAI_INTERACTIVE__*` env pattern for consistency with `interactive_mode`. **Owner:** Maintainer. **Impact:** Medium for docs.
3. **OQ-3:** Windows interactive terminals: is parity required in Phase 1? **Proposed default:** If current release targets Windows interactively, require parity; else document “best effort” under **FR-8** fallback. **Owner:** Maintainer. **Impact:** Medium for testing matrix.

## 10. Appendix (Optional)

- **Glossary:** **TTY** — interactive terminal; **builtin** — `exit`/`quit`/`help`/`?`/`reload` session commands.
- **Related:** Archived plan `plans/archive/interactive-mode/` for broader interactive session behavior.
