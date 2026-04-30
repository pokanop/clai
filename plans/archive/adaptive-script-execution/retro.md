<!-- PRD: plans/archive/adaptive-script-execution/prd.md -->
<!-- Tasks: plans/archive/adaptive-script-execution/tasks.md -->
<!-- Closed: 2026-04-28 -->
<!-- Archived: 2026-04-28 -->

# Retrospective: Adaptive script execution and runtime tool awareness

> Delivered phased runtime tooling detection, prompt and doctor surfaces, optional ephemeral script materialization with policy-safe cleanup, plus tests—without weakening the existing policy/cwd posture.

## Summary

The PRD tackled a mismatch between opaque host capability and brittle shell-heavy command proposals by adding structured PATH tooling visibility, richer system instructions for script-shaped answers, and a gated path that writes multi-line scripts to managed temp locations and tears them down on every exit path. Implementation followed the PRD’s two-phase split: Phase 1 shipped observability (`RuntimeTooling`, `[tooling]` config, shared `build_system_prompt`, doctor rows) while Phase 2 added `script_body` / extension on `CommandProposal`, materialization (`ephemeral_script`), session and `cmd_ask` integration, explicit pre-run/verbose cues, policy checks on materialized argv, and automated coverage including golden scenario strings.

All nine tasks completed; no `[!]` or `[-]` items remain. Phase 2 stayed behind **`ephemeral_scripts` default false** per PRD §9. One acceptance-style gap remains soft: **US-4(B)** calls out user-visible documentation of how cwd jail interacts with managed temp paths; behavior is enforced in code and exercised by tests, but there is **no README-level** rundown yet.

Outcome: Initiative goals **G-1–G-4**, success criteria **SC-1–SC-4**, and CI-style quality gates attached to code are met; residual risk is informational (documentation and informal NFR-1 benchmarking rather than regressions).

## Metrics

Task counts below use **top-level** task bullets from `tasks.md` only.

| Metric | Value |
|--------|-------|
| Total tasks | 9 |
| Completed `[x]` | 9 (100%) |
| Skipped `[-]` | 0 |
| Blocked `[!]` | 0 |
| Not started `[ ]` | 0 |
| Effective completion rate | **100%** (9 / (9 − 0)) |
| Phase 1 completion | 4 / 4 (100%) |
| Phase 2 completion | 5 / 5 (100%) |
| PRD labeled items in inventory¹ | **32** |
| PRD requirements fully addressed (engineering assessment)² | **31 / 32** (~97%) |
| PRD acceptance partially met | **1** (US-4(B) — see Scope Drift) |
| Tasks without PRD label traceability³ | **9** |

¹ Inventory: SC-1…SC-4, US-1…US-5, FR-1…FR-13, NFR-1…NFR-5, QG-1…QG-5 — excluding goals G-* as overlapping with FR/US.

² Mapped from shipped code/tests/plan artifacts; `tasks.md` does **not** list `FR-n`/`US-n` beside each task, so this percentage is substantive coverage, not an automated tasks↔label join.

³ **Decomposition style gap**, not scope creep: work maps cleanly to the PRD, but the task list prose does not back-link requirement IDs — future plans may add a Requirements row per task for metrics automation.

**Interpretation**: Completion metrics are maximal; coverage is one short on explicit user-facing narrative for cwd jail + managed temp interplay (see Section 4).

## What Was Built

For a reader without the PRD:

- **`RuntimeTooling` module** (`src/runtime_tooling.rs`): bounded PATH-style probes for common interpreter categories with a **process-lifetime cache** (`OnceLock`) when `detect_runtimes` is enabled; empty/stable snapshot when disabled.
- **`[tooling]` configuration** (`ToolingConfig`): `detect_runtimes`, `ephemeral_scripts` (default off), `prefer_scripts_when_available`; env overrides via `CLAI_TOOLING__*` figment layering.
- **System prompt** enrichment: tooling JSON embedded with guidance to prefer concise scripts where appropriate **without forcing** trivial one-shot argv; **`ask` and cloud paths share** the same built system string (`main.rs`).
- **`clai doctor`**: Runtime tooling row(s) reflecting presence/absence per category when detection is on, or explicitly “disabled”.
- **`CommandProposal`** extensions: optional `script_body` and `script_extension`; schema/tests and cloud **strict schema** wired through `schema_json()`.
- **Ephemeral script materialization** (`src/ephemeral_script.rs`): writes under a managed temp dir with **private file modes** where supported; strips body from proposals after substitution; rejects `script_body` when ephemeral scripts are disabled; **rejects host temp scripts under Docker execution** with a clear error.
- **Lifecycle hooks**: RAII holders plus **`discard_ephemeral_temp` before `process::exit`** and on policy-abort paths so cleanup is not reliant on dropping through the happy path alone.
- **Policy & UX**: Final argv (including temp path in args) flows through **`PolicyEngine`**; pre-run distinguishes **managed temp script** runs (`PreRunLine::ManagedTempScript`); verbose surfaces **artifact count/pattern** without dumping script bodies.
- **Interactive parity**: `session.rs` uses the same prepare/hold pattern as `cmd_ask`.
- **Tests**: Unit coverage for ephemeral materialization/rejection/config/schema/runtime tooling; **policy regression** (`strict_allowlist_sees_final_interpreter_with_temp_path`); **`tests/adaptive_script_golden.rs`** with ≥5 documented golden scenario strings (SC-4).

## Scope Drift

### Additions (built beyond the PRD)

*No material gold-plating.* One **productized guardrail** is called out explicitly in `decisions.md`: **blocking Docker mode when `script_body` is proposed** — it narrows FR-5 to a safe subset (host-visible temp files incompatible with typical container mounts) rather than implying cross-mount execution.

*(Per strict metrics semantics, all nine tasks omit inline `FR-N` citations in `tasks.md`; that’s a workflow/documentation gap for traceability tooling, not an implied “addition beyond PRD.”)*

### Deferrals (planned but deferred)

- **US-4 (acceptance criterion B)** — user-facing prose explaining **interaction between cwd jail and paths under managed temp contracts**: encoded in Rust behavior and regression tests (`cwd_jail`, policy tests), plus plan/decision notes, **not** summarized in **`README.md`**. Reason: implementation-first delivery; backlog as a tiny doc polish if support asks.

Residual **PRD §9 open questions**: resolved implicitly in shipped defaults (`detect_runtimes = true`, `ephemeral_scripts = false`, prompt parity defaults) documented in **`decisions.md`**; formal “cloud vs omit detection” ambiguity closed by sending **same local tooling snapshot**.

### Blockers (planned but blocked)

*None.*

## Key Decisions

### Runtime tooling struct vs widening `HostContext`

**Context**: PRD D-1 left room to extend host context vs a dedicated snapshot type.

**Decision**: Introduced **`RuntimeTooling`** in `runtime_tooling.rs`, kept separate from **`HostContext`**, merged/presented alongside in the prompt pipeline.

**Impact**: Stable, testable JSON for tooling probes; avoids conflating static host facts with mutable PATH probing.

See: `decisions.md` 2026-04-26 (“Runtime struct”).

### Cache vs default-off for detection (NFR-1 / FR-2)

**Context**: Risk of PATH probe latency.

**Decision**: **`OnceLock` process cache** when `detect_runtimes = true`; **configurable off** yields empty snapshot with no probing.

**Impact**: Honors NFR-1 via cheap repeated access; admins can forbid probes outright (NFR-2).

See: `decisions.md` 2026-04-26 (“Process cache”, “Config defaults”).

### `ephemeral_scripts` default false

**Context**: FR-6 / §9 open question — default on poses surprise vs conservative security posture.

**Decision**: **`ephemeral_scripts = false`** until explicitly enabled alongside Phase 2 materialization.

**Impact**: No silent new filesystem contract for existing users.

See: `decisions.md` 2026-04-26.

### Cargo `tempfile` + Unix `0600`; explicit discard before exit

**Context**: Reliable cleanup (US-3, FR-5, SC-1) vs `Drop` skipping on **`process::exit`**.

**Decision**: Promoted **`tempfile`** to a runtime dependency; private file modes; **`discard_ephemeral_temp`** invoked on abrupt exits/policy blocks post-materialization.

**Impact**: Narrower filesystem exposure; fewer leak reports under early termination.

See: `decisions.md` 2026-04-26; tasks 2.2 notes.

### Policy sees materialized argv; Docker + bodies rejected

**Context**: FR-7 and safe execution envelopes.

**Decision**: **`PolicyEngine` operates on interpreter + argv after temp substitution**; **Docker + `script_body`** errors cleanly.

**Impact**: Strict allowlists see real binaries/paths (`strict_allowlist_sees_final_interpreter_with_temp_path`); avoids misleading “successful” mounts that cannot see `/tmp`-style artifacts.

See: `decisions.md` 2026-04-26.

### Minor decisions (summary bullets)

- **Windows probing limits**: heuristic without exhaustive PATHEXT pass-through on bare commands — flagged under Future Opportunities (`decisions.md`).
- **`bwrap` + `/tmp` visibility**: consciously not special-cased — user reports may drive later.

## What Worked Well

- **Phased rollout in the PRD matched the codebase**: Phase 1 could ship value (prompt/doctor) before riskier filesystem work.
- **Strong alignment between policy tests and FR-7**: adding `strict_allowlist_sees_final_interpreter_with_temp_path` encodes regression signal where security-sensitive users notice drift first.
- **Single system-prompt builder** for local + cloud kept **FR-10** from becoming a drifting dual implementation.
- **Explicit “discard before exit” checklist** tackled a classic CLI foot-gun (`process::exit` bypassing RAII).
- **`decisions.md` plus task completion notes** form a workable mini-audit trail even without formal task→FR-ID lines.

## What to Improve

- **Requirement IDs in tasks**: Omitting `FR-n`/`US-n` mentions in `tasks.md` makes automated retros coverage math noisy; encode a Requirements line per task during `prd-to-tasks`.

- **NFR-1 evidence**: Implemented via caching, but **no recorded p95 measurement** artifact; lightweight benchmark or smoke note in-plan would seal the narrative for skeptics.

- **US-4(B) / operator docs**: Add a short **`README`** or **`docs/`** bullet on **[tooling]**, ephemeral scripts, cwd jail, and what appears in **`argv`** after substitution — lowers support load.

- **SC-1 keyword coverage (“user cancel”)**: Cleanup paths are exercised through policy/exit scaffolding; future interactive-cancel-specific integration test optional if regressions reported.

## Open Items

All tasks are closed (`[x]` everywhere). No `[!]` or incomplete `[ ]` tasks carry forward inside this folder.

Residual **non-task** follow-ups (optional backlog, not `[ ]` task rows):

| Item | Status | Next step |
|------|--------|-----------|
| End-user wording for cwd jail + managed temp interaction (US-4 B) | Open | Brief README/`docs/` addition — owner: docs or same implementer |

## Future Opportunities

From `decisions.md` and observations:

- **Bwrap visibility of `/tmp` scripts**: revisit if sandbox users confuse managed temp semantics with cwd jail binds.
- **Windows PATH / PATHEXT**: deeper resolution if Windows users report misses on interpreters without customary extensions tested today.
- **README integration**: unify configuration examples for **`[tooling]`** alongside existing execution-mode documentation.

---

_Archived_: **2026-04-28** — plan lives under **`plans/archive/adaptive-script-execution/`** (PRD, tasks, decisions, and this retro).
