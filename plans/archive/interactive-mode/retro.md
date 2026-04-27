<!-- PRD: plans/interactive-mode/prd.md -->
<!-- Tasks: plans/interactive-mode/tasks.md -->
<!-- Closed: 2026-04-26 -->

# Retrospective: Interactive mode for clai

> Phase 1 and Phase 2 of the interactive-mode plan shipped: default TTY session, warm local inference, pre-run presentation, tri-state execution, and polish including `ask` parity—without blocking non-TTY `clai` or changing script-facing `clai ask` contracts.

## Summary

The PRD targeted slow, repetitive cold-starts for `clai ask` and a subcommand-heavy entrypoint by making **bare `clai` on a TTY** the primary experience: one process, one local model load per session where applicable, a **rich pre-run explanation** (argv, intent, rationale, policy context) **before** any executor call, and **dry-run / confirm / auto** behavior with documented precedence. Implementation delivered that MVP plus Phase 2 items (in-session `help`, optional `reload`, and opt-in pre-run presentation for `clai ask` on TTY). Non-TTY no-arg invocation exits without hanging (exit **2** with a hint, per plan). `clai ask` and automation paths were preserved. Phase 3 (e.g. bounded multi-turn context) remains explicitly deferred per the PRD and task list, not a delivery gap for this closure.

## Metrics

| Metric | Value |
|--------|-------|
| Total tasks | 21 |
| Completed `[x]` | 21 (100% of total) |
| Skipped `[-]` | 0 |
| Blocked `[!]` | 0 |
| Not started `[ ]` | 0 |
| Effective completion rate | 100% (completed / (total − skipped)) |
| PRD requirements covered (labeled FR/NFR/US/SC/QG in task coverage) | 47 / 47 (100%) |
| Tasks without PRD traceability | 0 |
| PRD requirements never implemented (within Phase 1–2 scope) | 0 |

All top-level tasks completed; no skipped or blocked rows in `tasks.md`.

## What Was Built

- **Default routing:** TTY + no subcommand/args → interactive session; optional `clai interactive` alias; non-TTY prints usage/hint and exits **2** (integration-tested).
- **Tri-state execution:** `dry-run` / `confirm` / `auto` with **CLI > env > config > default** resolution; legacy `policy.dry_run_default` maps to interactive dry-run when the new key is absent; `--yes` documented for default session.
- **Session loop:** Welcome/status, distinct input prompt, EOF and `exit`/`quit`, non-fatal errors keep the session alive, built-in `help` and optional `reload` (Phase 2).
- **Warm local inference:** Session-scoped backend/model reuse (NFR-1); `clai ask` kept on a thin path without regressing one-shot behavior.
- **Cloud:** Same completion contract as `cmd_ask` per line; no agent pooling (documented as best-effort).
- **Pre-run presentation:** Structured block before executor (including dry-run); system prompt nudges `reason`; blocked proposals show explanation, no run offer.
- **FR-16 ordering:** Presentation → policy sensitive confirm (when applicable) → interactive “run it?” in confirm mode → execution; dry-run skips execution and related prompts per implemented interpretation.
- **Output:** TTY severity styling with **NO_COLOR** support (manual ANSI in-project).
- **Operator UX:** `doctor` shows effective interactive mode from config+env (CLI overrides noted as not applied in `doctor`).
- **Docs/tests:** README/CHANGELOG updates; unit tests for mode resolution, presentation, ordering, built-ins, `NO_COLOR`; non-TTY integration test; quality gates satisfied (including `cargo fmt --check`; CI includes fmt).

## Scope Drift

### Additions (built beyond the PRD)

No additions beyond PRD scope. Implementation choices (e.g. manual `tty` styling instead of a new crate) are implementation detail, not extra product scope.

### Deferrals (planned but deferred)

- **PRD Phase 3 / future PRD:** Bounded multi-turn context, full-screen TUI, daemon-style sharing—explicitly **out of scope** for this task list and PRD non-goals for v1; **not** treated as skipped `[−]` tasks.

### Blockers (planned but blocked)

None.

## Key Decisions

### Keep config version at 1 with serde defaults
**Context:** FR-19 and migration risk for existing installs.  
**Decision:** No `CONFIG_VERSION_LATEST` bump; `[interactive]` uses `serde(default)` so old configs load cleanly.  
**Impact:** Future plans should not assume a version bump for every new nested table—defaulting patterns may suffice.  
See: `decisions.md` 2026-04-26 / Task 1.2

### Manual ANSI styling (no new color crate)
**Context:** PRD allowed a small styling dependency; dependency budget and licensing.  
**Decision:** Implement `NO_COLOR`/`IsTerminal`-gated escapes in `src/tty.rs` instead of adding e.g. `anstream`/`owo-colors`.  
**Impact:** Less dependency surface; maintainers own ANSI correctness across platforms.  
See: `decisions.md` 2026-04-26 / Task 1.5

### `doctor` reflects config+env only (not CLI overrides)
**Context:** `clai doctor` does not accept `--interactive-mode` / `--yes`.  
**Decision:** Document that effective mode shown is from config and env, not process CLI flags.  
**Impact:** Operators should use docs or a test invocation for full process effective mode when flags are used.  
See: `decisions.md` 2026-04-26 / Task 1.13

### Session vs one-shot child exit semantics
**Context:** UX for iterative loop vs single-shot `ask`.  
**Decision:** After a child run, session stays open; non-zero child exit is surfaced in-session (e.g. warning mapping), not as process exit code—unlike one-shot `clai ask`.  
**Impact:** Scripts still use `clai ask`; interactive users can continue after a failed command.  
See: `decisions.md` 2026-04-26 / Task 1.10 / 1.11

### Interactive dry-run and FR-16
**Context:** Ordering of policy sensitive confirm vs dry-run.  
**Decision:** Interactive `dry-run` skips policy sensitive confirmation and execution prompts (steps 2–4), aligned with PRD FR-16 for dry-run.  
**Impact:** Dry-run remains “never execute” without extra confirm friction.  
See: `decisions.md` 2026-04-26 / Task 1.11

## What Worked Well

- **Phased tasks (Phase 1 MVP → Phase 2 polish)** kept shipping boundaries clear; Phase 3 stayed a clean “future” bucket.
- **Explicit requirements column (FR/US/NFR/SC/QG) on every task** made coverage and retro tracing straightforward.
- **`decisions.md`** captured small but important behavioral deltas (doctor, child exit, dry-run ordering) that are easy to lose in code-only history.
- **Non-TTY integration test** directly mitigated the PRD’s “CI hang” risk for bare `clai`.
- **Engine/session split** (warm load in engine, loop in session) matched the PRD architecture and kept `ask` on a thin path.

## What to Improve

- **Cloud latency:** Stateless `complete_cloud` per line matches `ask` but leaves room for a follow-on spike if operators care about connection reuse (already noted in decisions).
- **Signal UX:** Ctrl+C during `read_line` / generation could be richer; currently more documentation than deep signal integration.
- **Expect-style E2E** for full interactive flows remains optional (PRD §7); if regressions appear, consider a minimal scripted smoke after meaningful UX changes.

## Open Items

All tasks are closed.

## Future Opportunities

- Optional `ureq::Agent` (or similar) for cloud session lines if latency matters.
- Richer signal handling for Ctrl+C during blocking read or generation.
- PRD Phase 3: bounded multi-turn context under a new PRD if product wants conversational follow-ups without unbounded memory (NFR-6 long-term).
- Scripted `expect`-style E2E for interactive flows if maintainers accept maintenance/flake tradeoffs.
