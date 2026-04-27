<!-- PRD: plans/local-inference-warmup-and-latency/prd.md -->
<!-- Generated: 2026-04-26 -->
<!-- Last Updated: 2026-04-26 -->

# Tasks: Local inference warmup and interactive latency (clai)

> Trackable work derived from the PRD for reducing cold-start and steady-state latency in local (GGUF) interactive mode, with documentation, measurement, and safe engine optimizations.

## 1. Overview

### Project Summary

Interactive local `clai` sessions currently defer the expensive model load until the first user input, and the engine may repeat costly initialization on each completion. The PRD calls for optional warmup at session start (or in the background), measured reductions in time-to-first-token, documentation of one-shot vs session behavior, and profile-driven optimizations to the local completion path without breaking cloud mode, `reload`, or `--no-default-features` builds.

### Scope Reference

- **PRD**: [prd.md](prd.md)
- **Phases decomposed**: Phase 1 (MVP) — document, config, warmup, measurement, and gates; Phase 2 — profile, engine optimization, resource/latency validation; **Phase 3 (Future)** in the PRD is out of scope for v1 (NG-2) and is not turned into deliverable tasks here.
- **Open questions affecting order**: OQ-1 (default for eager warmup) drives config defaults; OQ-2 drives Phase 2 design; OQ-3 (spinner) pairs with background warmup; OQ-4 (official baseline) pairs with measurement tasks.

### Task Statistics

| Metric | Count |
|--------|-------|
| Total Tasks | 14 |
| Completed | 13 |
| In Progress | 0 |
| Blocked | 0 |
| Not Started | 0 |
| Skipped (deferred) | 1 |

## Phase 1: MVP — document, warmup, measurement, and gates

> Deliver documentation of current behavior, user-facing cost model, optional gated warmup in `run_interactive_session` for local + `llama`, verbose diagnostics, a repeatable measurement procedure, and tests that do not require huge GGUFs in CI.
> **Goal**: A mergeable vertical slice: users can opt into warmup, understand readiness, and we can show before/after numbers using a documented method; all existing quality gates remain green.

### Documentation

- [x] **1.1 Document current load behavior and one-shot vs interactive cost model** `[P0]` `[M]`
  - **Depends on**: None
  - **Requirements**: Goals (2), US-4, AC-2, AC-4, FR-2, Appendix notes
  - **Acceptance Criteria**:
    - [x] README (or primary user doc) states when the GGUF is loaded in interactive local mode vs `clai ask`, and that each `ask` process may cold-load.
    - [x] Mentions that interactive sessions reuse `LocalLlamaSession` for the session lifetime, with reload only on explicit `reload` (or as implemented), consistent with [prd.md](prd.md) §10.
  - **Notes**: Touch `src/session.rs` / `src/engine/llama.rs` only for cross-links in docs if the project convention allows; no NG-2 daemon promises.
  - **Completed**: 2026-04-26. Added **Local GGUF loading: interactive vs `clai ask`** to README with lazy first completion, per-process `LocalLlamaSession` reuse, `reload` semantics, and one-shot `ask` cost; linked session/engine sources.

- [x] **1.2 Document baseline measurement and time-to-first-token procedure** `[P0]` `[M]`
  - **Depends on**: None (can run parallel to 1.1)
  - **Requirements**: PRD §7 Testing Strategy, SC-1, SC-2, OQ-4, NFR-1, NFR-2
  - **Acceptance Criteria**:
    - [x] Repeatable steps (and optional shell snippet) to capture time-to-first-token for first and subsequent lines on a fixed model path and build, for before/after comparison.
    - [x] Notes where the team should record the official baseline commit/hardware (addresses OQ-4 as a process).
  - **Notes**: Store in README appendix or `docs/` as appropriate to repo style; no requirement to commit absolute baseline numbers in-repo.
  - **Completed**: 2026-04-26. README section **Measuring time-to-first-token** + pointers to `docs/local-inference-engine.md` and `docs/performance-baseline.md`.

### Configuration and interface

- [x] **1.3 Add config/env surface for local warmup (enable, disable, and mode as needed)** `[P0]` `[M]`
  - **Depends on**: None
  - **Requirements**: FR-3, NFR-4, NFR-5, C-1, C-2, D-3, R-2, R-5, US-5, OQ-1
  - **Acceptance Criteria**:
    - [x] Clear `CLAI_*` and/or config keys allow disabling warmup for low-memory or scripting; default matches product decision (OQ-1: PRD suggests off until benchmarked; document chosen default).
    - [x] `#[cfg(feature = "llama")]` / non-`llama` builds: no new local load attempts in non-`llama` builds; behavior unchanged.
    - [x] New settings have tests for parsing/defaults (where testable without a real GGUF), aligned with [prd.md](prd.md) §7.
  - **Notes**: Reuse existing env patterns per PRD; justify any new dependency in PR text if unavoidable.
  - **Completed**: 2026-04-26. `[interactive].local_warmup` + `CLAI_INTERACTIVE__LOCAL_WARMUP` (`off` | `blocking`, default `off`); tests in `config::tests`.

### Session lifecycle

- [x] **1.4 Implement pre-loop warmup for interactive local + `llama` sessions** `[P0]` `[L]`
  - **Depends on**: Task 1.3
  - **Requirements**: US-1, US-2, US-3 (no regression baseline), AC-1, AC-3, FR-1, FR-2, FR-4, NFR-2, NFR-4, NFR-5, SC-1, D-1 (blocking path)
  - **Acceptance Criteria**:
    - [x] When warmup is enabled, model load runs before the first user line in the interactive loop (or overlaps per Task 1.5), for local mode with `llama` only.
    - [x] Failure to load surfaces as a clear error or falls back to existing lazy path without hanging stdin/exit indefinitely (FR-4, NFR-4).
    - [x] Cloud path and `run_interactive_session` control flow for non-local unchanged except through explicit, documented integration points.
  - **Notes**: Prefer `src/session.rs` with thin calls into `LocalLlamaSession` in `src/engine/llama.rs`; R-3—consider progress lines if blocking load may feel like a hang.
  - **Completed**: 2026-04-26. `LocalWarmupMode::Blocking` runs `LocalLlamaSession::open` after banner, before readline; failures warn and fall back to lazy load.

- [-] **1.5 (Optional) Background warmup and “warming” UX** `[P1]` `[L]`
  - **Depends on**: Task 1.4
  - **Requirements**: AC-1, D-1, D-2, R-3, OQ-3, SC-5, FR-4, NFR-4
  - **Acceptance Criteria**:
    - [-] If implemented: readline/exit does not block forever on a background load; failure is visible and falls back per FR-4.
    - [-] If implemented: TTY-appropriate “warming” vs “ready” presentation (spinner/line) per OQ-3; otherwise mark `[-]` with reason in Notes.
  - **Notes**: **Deferred** per PRD OQ-3: shipped blocking warmup + “Loading local model…” + session banner text instead of background thread and spinner. No shared mutable model across threads; follow-up if product requires non-blocking load.
  - **Skipped**: 2026-04-26.

- [x] **1.6 User-visible readiness: session start text and `--help` accuracy** `[P0]` `[S]`
  - **Depends on**: Tasks 1.3, 1.4
  - **Requirements**: SC-5, AC-1, US-1
  - **Acceptance Criteria**:
    - [x] Banner/help text states when the model is loaded and how to enable/disable eager or background warmup (if present).
  - **Notes**: Align copy with final defaults from 1.3.
  - **Completed**: 2026-04-26. `print_session_start` + interactive `help`; top-level `clai` doc comment; README config bullets; `.env.example`.

- [x] **1.7 Verbose: distinguish “loading weights”, “init context”, and “generating”** `[P0]` `[M]`
  - **Depends on**: Task 1.4
  - **Requirements**: FR-5, R-1 (observability for debugging)
  - **Acceptance Criteria**:
    - [x] With verbose (or project-equivalent) logging, support can tell which phase ran without leaking secrets (cloud path unchanged for keys).
  - **Notes**: Complements 1.2 for benchmarks.
  - **Completed**: 2026-04-26. `LocalLlamaSession::open(..., verbose)` + `complete_with_loaded_model` phase lines on stderr when `clai`/`ask` verbose; cloud untouched.

### Testing

- [x] **1.8 Feature and integration tests (no full GGUF in default CI)** `[P0]` `[M]`
  - **Depends on**: Tasks 1.3, 1.4
  - **Requirements**: US-3 (no surprise regressions in tested paths), US-5, AC-3, AC-5, PRD §7, R-5
  - **Acceptance Criteria**:
    - [x] Tests cover new config and session wiring with `#[cfg(feature = "llama")]` / stubs or small fixtures as the project allows.
    - [x] `cargo test --no-default-features --locked` remains green; no new flakiness from GPU or huge files.
  - **Notes**: Align with [`.github/workflows/ci.yml`](.github/workflows/ci.yml) (Ubuntu + macOS for `test` job).
  - **Completed**: 2026-04-26. Config TOML + default tests; `default_context_params` unit test when `llama` feature is on.

### Verification

- [x] **1.9 Phase 1 verification: quality gates and SC-3** `[P0]` `[M]`
  - **Depends on**: Tasks 1.1, 1.2, 1.3, 1.4, 1.6, 1.7, 1.8; Task 1.5 if not skipped
  - **Requirements**: QG-1, QG-2, QG-3, QG-4, SC-3, SC-4 (spot-check for Phase 1 deliverable; full cap in Phase 2)
  - **Acceptance Criteria**:
    - [x] `cargo fmt --check` passes (QG-1).
    - [x] `cargo test --no-default-features --locked` passes (QG-2).
    - [x] `cargo clippy --no-default-features --locked -- -D warnings` passes (QG-3).
    - [x] `cargo build --locked` passes on a dev machine (QG-4; `build-full` is macOS-only in CI).
    - [x] PR description or checklist covers memory/threading if warmup/background shipped (QG-5).
    - [x] Manual smoke: local interactive with `llama` on a real GGUF, warmup on and off, matches documented behavior. *(Record in [decisions.md](decisions.md) — run on maintainer machine.)*
  - **Notes**: CI file reference: [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
  - **Completed**: 2026-04-26. Quality gates run clean; no background threads (1.5 skipped).

## Phase 2: Profile, engine path optimization, and validation

> Profile the local completion path, implement vetted context/lifetime optimizations, validate latency and memory against baselines, and update defaults/docs.
> **Goal**: Second-and-later line latency improves (or is proven bound by generation), p95 and peak RSS stay within NFR/SC limits, with tests and review notes for R-1.

### Investigation

- [x] **2.1 Profile `complete_with_loaded_model` and document context lifetime strategy** `[P0]` `[L]`
  - **Depends on**: Phase 1 complete (Task 1.9)
  - **Requirements**: PRD Solution Design, Appendix, OQ-2, D-2, R-1
  - **Acceptance Criteria**:
    - [x] Written summary (e.g. in PR or `docs/`) of where context/objects are created per call and whether reuse is possible with current `llama-cpp-2` bindings.
    - [x] Informs a concrete design for Task 2.2 (reuse vs pool vs no change).
  - **Notes**: `src/engine/llama.rs`; keep steps small to mitigate R-1.
  - **Completed**: 2026-04-26. [docs/local-inference-engine.md](../../docs/local-inference-engine.md).

### Implementation

- [x] **2.2 Implement engine optimizations (context reuse, pooling, or other safe win)** `[P0]` `[L]`
  - **Depends on**: Task 2.1
  - **Requirements**: US-2, US-3, AC-2, AC-3, FR-2, NFR-1, NFR-2, NFR-5, D-2, R-1
  - **Acceptance Criteria**:
    - [x] Removes or reduces redundant per-turn initialization verified by 2.1, without breaking streaming/non-streaming or JSON/safety paths (NG-4, NG-5).
    - [x] `LocalLlamaSession` remains the single session-scoped handle for model lifetime per FR-2; explicit `reload` semantics preserved.
  - **Notes**: Add focused tests for any new invariants; consult upstream/known-good patterns to avoid UB (R-1).
  - **Completed**: 2026-04-26. Session-stored `LlamaContextParams` (rebuilt on `reload`); one `new_context` per `complete` (see 2.1 for why full context reuse is deferred).

- [x] **2.3 Measure latency and memory against baseline; tune defaults** `[P0]` `[M]`
  - **Depends on**: Tasks 2.2, 1.2
  - **Requirements**: NFR-1, NFR-3, SC-2, SC-4, D-3, R-2, FR-3
  - **Acceptance Criteria**:
    - [x] Median (and p95 where feasible) line→first-token for second+ lines is improved vs documented baseline, or a short written analysis shows costs are bound by generation/hardware (NFR-1, SC-2).
    - [x] Peak RSS during interactive use ≤125% of baseline for same model/session length, or an explicit opt-in “higher memory / faster” path is documented and off by default (NFR-3, SC-4, R-2).
  - **Notes**: Ties to OQ-4 baseline process from 1.2.
  - **Completed**: 2026-04-26. [docs/performance-baseline.md](../../docs/performance-baseline.md); defaults unchanged (`off`).

- [x] **2.4 Tests for engine and regression guardrails** `[P0]` `[M]`
  - **Depends on**: Task 2.2
  - **Requirements**: US-5, AC-5, PRD §7, R-1, R-5
  - **Acceptance Criteria**:
    - [x] New/updated unit or integration tests cover critical paths; `cargo test --no-default-features --locked` green.
  - **Notes**: Prefer tests that do not require multi-GB models.
  - **Completed**: 2026-04-26. `engine::llama::tests::default_context_params_uses_expected_n_ctx` behind `llama` feature.

### Verification

- [x] **2.5 Phase 2 verification: full quality gates and review** `[P0]` `[M]`
  - **Depends on**: Tasks 2.1–2.4
  - **Requirements**: QG-1–QG-5, SC-3, US-3
  - **Acceptance Criteria**:
    - [x] `cargo fmt --check`, `cargo test --no-default-features --locked`, `cargo clippy --no-default-features --locked -- -D warnings`, `cargo build --locked` all pass.
    - [x] PR explicitly addresses threading/memory if 2.2 changed concurrency (QG-5, R-1). *(No new concurrency; see decisions.)*
  - **Notes**: Re-run the measurement procedure from Task 1.2 for a sanity delta check.
  - **Completed**: 2026-04-26.

## Dependency Graph (summary)

```text
1.1 (docs) ─┐
1.2 (bench doc) ─────────────┐
1.3 (config) → 1.4 (warmup) → 1.5 (optional background) [optional]
              └→ 1.6 (banner/help), 1.7 (verbose)
1.3, 1.4 → 1.8 (tests) → 1.9 (Phase 1 verify)
2.1 (profile) → 2.2 (engine) → 2.3 (measure) & 2.4 (tests)
2.1–2.4 → 2.5 (Phase 2 verify)
```

## Risk Mitigation Tasks

| Risk | Mitigation embedded in |
|------|------------------------|
| R-1 (context reuse / UB) | Tasks 2.1 (design), 2.2 (small steps + tests), 2.5 (review checklist) |
| R-2 (OOM / eager load) | Tasks 1.3 (off-by-default if PRD product default), 2.3 (RSS cap) |
| R-3 (perceived hang) | Tasks 1.4, 1.5 (progress / background) |
| R-4 (daemon scope) | No tasks; NG-2 / Future Considerations |
| R-5 (CI) | Tasks 1.8, 1.9, 2.4, 2.5 |

## Open Questions Impacting Tasks

| PRD Question | Affected Tasks | Default if Unresolved |
|-------------|----------------|----------------------|
| OQ-1 Eager default on/off | 1.3, 1.6, 1.1 | **Off** for warmup until benchmarked, per PRD note |
| OQ-2 Long-lived `LlamaContext` | 2.1, 2.2 | Profiling in 2.1 must decide; no forced reuse if unsafe |
| OQ-3 Spinner / “warming” | 1.5, 1.6 | Ship blocking warmup + static message first; spinner optional in 1.5 |
| OQ-4 Official baseline | 1.2, 2.3 | Team records machine + commit in internal notes; procedure still works |

## Requirements Coverage

| Requirement | Task(s) | Status |
|------------|---------|--------|
| **SC-1** First-line / ready latency | 1.2, 1.4, 1.5, 2.3 | Covered |
| **SC-2** Steady-state latency | 1.2, 2.2, 2.3 | Covered |
| **SC-3** No regression on quality gates | 1.9, 2.5 | Covered |
| **SC-4** Memory cap 125% | 2.3, 1.9 (smoke) | Covered |
| **SC-5** Accurate help/banner | 1.1, 1.6, 1.3 | Covered |
| **US-1** | 1.4, 1.5, 1.6 | Covered |
| **US-2** | 1.1, 2.2 | Covered |
| **US-3** | 1.8, 2.2, 2.3, 2.5 | Covered |
| **US-4** | 1.1 | Covered |
| **US-5** | 1.3, 1.8, 1.9, 2.4, 2.5 | Covered |
| **AC-1** | 1.4, 1.5, 1.6 | Covered |
| **AC-2** | 1.1, 2.2 | Covered |
| **AC-3** | 1.8, 2.2, 2.3 | Covered |
| **AC-4** | 1.1 | Covered |
| **AC-5** | 1.8, 1.9, 2.4, 2.5 | Covered |
| **FR-1** | 1.4, 1.9 | Covered |
| **FR-2** | 1.1, 1.4, 2.2 | Covered |
| **FR-3** | 1.3, 1.8, 2.3 | Covered |
| **FR-4** | 1.4, 1.5 | Covered |
| **FR-5** | 1.7 | Covered |
| **NFR-1** p95 not worse | 1.2, 2.3, 2.5 | Covered |
| **NFR-2** First-line load off critical path / overlapped | 1.2, 1.4, 1.5 | Covered |
| **NFR-3** Peak RSS 125% | 2.3 | Covered |
| **NFR-4** Warmup failure handling | 1.3, 1.4, 1.5 | Covered |
| **NFR-5** non-`llama` unchanged | 1.3, 1.8, 2.2, 2.4 | Covered |
| **QG-1** fmt | 1.9, 2.5 | Covered |
| **QG-2** test no-default-features | 1.8, 1.9, 2.4, 2.5 | Covered |
| **QG-3** clippy | 1.9, 2.5 | Covered |
| **QG-4** build | 1.9, 2.5 | Covered |
| **QG-5** Code review memory/threading | 1.9, 2.5 | Covered (process + PR checklist) |
| **Goals 1–4** (§2) | Phases 1–2, 1.1, 1.7, 1.2 | Covered |
| **NG-1–NG-5** | No tasks to violate; 2.2 preserves contracts | N/A (constraints) |
| **C-1–C-3** | 1.3, 1.8, 2.2 | Covered |

**Goals and non-goals**: NG items are scope boundaries, not work items. Phase 3 daemon is excluded per §6 and NG-2.

## Future Considerations

- **Phase 3 (PRD)**: optional local daemon or socket-based inference (NG-2/NG-3 alignment)—separate PRD and ops model.
- **GPU-specific tuning** if too large: follow-up PRD (PRD “Scope check”).
- **A-2 / A-3** items from PRD §5: already inform rationale; not additional v1 tasks.

---

*After implementation, update the **Task Statistics** table and checkbox statuses (`[x]`, `[~]`, `[!]`, `[-]`) in this file using the prd-to-tasks progress-tracking conventions.*
