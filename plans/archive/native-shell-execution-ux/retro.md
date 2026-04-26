<!-- PRD: plans/archive/native-shell-execution-ux/prd.md -->
<!-- Tasks: plans/archive/native-shell-execution-ux/tasks.md -->
<!-- Closed: 2026-04-26 -->

# Retrospective: Native shell execution UX for `clai ask`

> `clai ask` was refactored so the default path behaves like a normal shell run (inherited TTY I/O in direct+human+TTY, child-aligned exit codes, minimal pre/post scaffolding) with verbose/structured output and non-direct attribution opt-in; Phase 2 added operator flags and stronger docs/tests.

## 1. Summary

The PRD called out a mismatch: default `ask` felt like a JSON “tool report” with piped I/O, not like typing the command in the terminal, and `clai`’s exit code did not follow the child. The team delivered a **stream strategy** (`inherit` vs `capture`) wired through the **executor**, **exit mapping** for `ask`, **default vs verbose** presentation, **non-direct** (Docker/bwrap) attribution, **integration tests** on the non-TTY direct path, **migration and manual TTY** documentation, and a **no shell snippets in repo** check for Phase 1. **Phase 2** layered README hardening, `--force-capture` and `--no-preview`, and edge-case tests (large output, timeout, non-UTF-8, policy). Outcome: **all 19 listed tasks are `[x]`**, with **0** blocked, skipped, or not started, and the PRD’s committed phases through Phase 2 are **implemented in code and verified** per the plan’s quality-gate notes.

## 2. Metrics

| Metric | Value |
|--------|--------|
| Total tasks | 19 |
| Completed `[x]` | 19 (100%) |
| In Progress `[~]` | 0 |
| Blocked `[!]` | 0 |
| Skipped `[-]` | 0 |
| Not started `[ ]` | 0 |
| **Effective completion rate** | **100%** (19 / (19 − 0)) |

**Phase breakdown**

- **Phase 1 (MVP):** 13/13 complete → **100%**
- **Phase 2:** 6/6 complete → **100%**

| Metric | Value |
|--------|--------|
| PRD requirements (inventory: SC-1..4, US-1..5, FR-1..6, NFR-1..4, QG-1..5) | 24 |
| Requirements with completed task coverage (per `tasks.md` §6) | 24 (100%) |
| Tasks without formal `FR-`/`US-`/`NFR-`/`QG-`/`SC-` in **Requirements** | 0 (Phase 2 tasks still tie to PRD Phase 2 or risks) |
| PRD requirements with **no** task in `tasks.md` | **0** |
| **Additions beyond PRD** (completed work with no PRD intent) | **0** |

*Interpretation: 100% effective completion and full coverage in the plan’s own matrix; closed scope with no `[-]`/`[!]` tasks.*

## 3. What Was Built

- **Stream selection** for direct runs: when to **inherit** stdio (human + TTY on all of stdin/stdout/stderr) vs **pipe capture** (verbose, non-TTY, non-direct, or forced capture).
- **Executor** changes: `StreamStrategy` on `run_proposal`; inherit path with empty captured strings; timeout/kill behavior preserved.
- **`clai ask` exit codes** aligned to the child where appropriate; dedicated codes for timeout (124), user decline (2), dry-run (3); policy still surfaces as error/exit 1; documented signal behavior.
- **Default human output:** at most one `Run: …` preview on TTY; post-exec raw streams on capture, none extra on inherit; **verbose** keeps structured blocks.
- **Non-direct** runs: one-line and verbose **context** (profile, cwd, argv, docker image) so output is attributable.
- **Tests:** unit tests for strategy and exit mapping; `tests/direct_path_exit_propagation.rs`, `trivial_child_overhead.rs`, `phase2_edge_cases.rs` and related.
- **Docs:** README migration, TTY manual checklist, limits/troubleshooting, Phase 2 flags; **CHANGELOG** Unreleased notes.
- **Operator flags:** `--force-capture`, `--no-preview` (+ config/env mirrors).
- **No first-party `zsh`/`fish`/`nu` paste-in** in repo deliverables (audit task).

## 4. Scope Drift

### Additions (built beyond the PRD)

*No additions beyond PRD scope* — completed work maps to the PRD’s Phase 1–2 intent and the task list; Phase 2 items were explicitly in the PRD’s phased rollout.

### Deferrals (planned in PRD but not in this plan cycle)

*All PRD requirements that this task list took on are implemented* — **Phase 3** items (shell snippets, `needs_shell`, CI PTY, etc.) stay **out of this plan** by design (PRD §6), not as failed tasks.

### Blockers (planned but blocked)

*None* — no `[!]` tasks.

## 5. Key Decisions

### All three stdio TTYs required for inherit

**Context:** Defining when “user terminal” means full inheritance.  
**Decision:** Inherit only when **stdin, stdout, and stderr** are all TTYs (`all_streams_tty`), avoiding half-interactive cases (e.g. piped stdin).  
**Impact:** Piped stdin in an otherwise interactive terminal does **not** get inherit; a future product choice could relax to stdout+stderr only (see `decisions.md` Future opportunities).  
*See: decisions.md 2026-04-26 / Task 1.1*

### `RunOutcome` empty strings on inherit

**Context:** How to represent captured output when the child wrote to the terminal.  
**Decision:** `stdout`/`stderr` in `RunOutcome` stay **empty** on inherit; status and `timed_out` carry the result.  
**Impact:** Callers must not expect captured text on the inherit path.  
*See: Task 1.2*

### Child exit = process exit; dedicated codes for non-child paths

**Context:** US-2 vs FR-4 (no fake “success” on abort).  
**Decision:** Map child exit to `clai` exit for completed runs; use **2** (decline), **3** (dry-run), **124** (timeout), **1** (policy), keep **`--print-only` → 0** with documented “not a child code” meaning.  
**Impact:** Script authors have predictable semantics; `print-only` remains distinct from dry-run.  
*See: Tasks 1.3, 1.6*

### Verbose and force-capture as explicit overrides

**Context:** FR-1 and operator need for capture under TTY.  
**Decision:** `OutputIntent::Verbose` and **`--force-capture`** (plus config/env) **force capture** on direct when needed; no legacy “old behavior” flag per PRD §9.  
**Impact:** Audits and size/timeout limits remain reachable without changing policy.  
*See: Tasks 1.4, 2.2*

**Minor decisions (condensed):** `non_direct` attribution without `reason` in lines (FR-5); integration tests stay **non-PTY** per NFR-1/Phase 1; Phase 2 edge tests document pipe/truncation limits (`decisions.md` 2.4).

## 6. What Worked Well

- **Clear layering:** `stream_strategy` as a testable module separate from policy and argv construction kept the matrix small and reviewable.
- **Phased delivery:** Phase 1 shippable core (default UX + exit + tests + migration) and Phase 2 as **flags + docs + edge tests** limited risk.
- **Decisions log** in `decisions.md` gives a durable rationale trail (TTY rule, `print-only` vs dry-run, non-UTF-8 verbose warning).
- **CI-friendly tests** (non-TTY, executor-level) without blocking on a live LLM or PTY in Phase 1.
- **Quality gates** recorded in task 1.13 / 2.6 notes (including `clippy --all-targets` for integration tests).

## 7. What to Improve

- **QG-4 vs CI:** Task 1.13 notes that **`cargo fmt --check`** may not be in `.github/workflows/ci.yml` — `tasks.md` “Future considerations” still recommend adding it so the repo and CI always match the PRD’s fmt gate.
- **Strict requirement IDs on every task line:** A few Phase 2 tasks use “PRD Phase 2 / Risk” phrasing; fine for humans, but for automation/metrics, prefer a consistent **`FR-` / `NFR-`** pointer where possible in future plans.
- **Phase 3** is named in the PRD but not turned into a follow-up `tasks.md` yet — when starting it, a short PRD or task stub avoids ambiguity between “deferred by design” and “forgotten.”

## 8. Open Items

**All tasks are closed** — there are no `[!]` or remaining `[ ]` items in this plan.

Carry-forwards (not unclosed tasks) live under **Future opportunities** and PRD §6 Phase 3.

## 9. Future Opportunities

- **Relaxed TTY rule** (e.g. inherit when only stdout+stderr are TTYs) if product wants it — **document in `decisions.md` when changed** (`decisions.md` *Future opportunities*).
- **CI:** add `cargo fmt --check` to the workflow; optional **pty** tests in a **later** PRD/phase.
- **Phase 3** per PRD: first-party shell snippets, `needs_shell`, clipboard, **CI PTY** — out of this plan’s committed tasks.
