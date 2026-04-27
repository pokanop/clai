# PRD: Local inference warmup and interactive latency (clai)

## 1. Executive Summary

- **Problem Statement**: In local (GGUF) mode, the interactive REPL currently defers the expensive model load until the first user input, so the first completion pays full cold-start cost while users expect a “ready” session after launch. Additionally, per-turn latency after weights are loaded may still be higher than necessary because the implementation may repeat initialization work on every completion within the same session. Separately, repeated `clai ask` invocations pay a full load each time, which is expected for one-shots but painful for back-to-back automation unless documented or optimized.
- **Proposed Solution**: Treat interactive local sessions as long-lived: optionally warm the model (and, where safe, other expensive setup) at session start or in the background so typical REPL use sees fast first and subsequent requests. Complement that by measuring and reducing redundant per-request work in the local inference path while preserving correctness, policy, and the existing cloud/local split. Prioritize changes that do not require new runtime dependencies without explicit justification.
- **Success Criteria**:
  - **SC-1**: After a successful `clai interactive` start in local mode, **time to first model-ready state** (user-visible: session reports ready or a documented progress indicator finishes) is **at least 50% lower than baseline** for the same machine, model file, and build, *or* the first user-submitted line no longer incurs the full GGUF load cost (whichever the implementation chooses to optimize).
  - **SC-2**: For a given interactive local session, **median wall-clock time from submitting a line to first streamed token** on the **second and later** user lines improves by **at least 20%** vs baseline on the same hardware, model, and prompt class, *or* a documented analysis shows the remaining cost is bound by unavoidable generation and external limits.
  - **SC-3**: No regression: **all existing quality gates** (see Section 7) pass on the main branch with the same CI matrix strategy as today.
  - **SC-4**: **Memory high-water** during interactive use does not exceed **125% of baseline** peak for the same session length and model without explicit user opt-in to a “higher memory / faster” mode.
  - **SC-5**: User-facing help or session banner text accurately describes when the model is loaded and how to opt into any background or eager warmup, if such options exist.

## 2. Goals and Non-Goals

### Goals

1. Reduce perceived and measured latency for local inference in the **interactive** workflow, with emphasis on the **first** user turn after session start and on **steady-state** REPL use.
2. Document current load behavior (what is reused across turns, what is re-created) so expectations match implementation.
3. Identify and implement **safe** optimizations in the local inference path that reduce redundant work per completion within a session, validated by tests or micro-benchmarks as appropriate.
4. Keep **feature parity** for cloud mode and for `reload` / model path resolution; changes must not break policy, JSON schema output, or execution flows.

### Non-Goals

- **NG-1**: This initiative will not replace llama.cpp or switch to a different local inference stack unless separately approved.
- **NG-2**: It will not add a persistent model daemon or cross-process model sharing (e.g. a background service used by every `clai` invocation) in the initial delivery; that is a possible future architecture.
- **NG-3**: It will not guarantee low latency for **one-shot** `clai ask` across **separate OS processes** without a separate design; each invocation may still cold-load by default.
- **NG-4**: It will not change cloud provider APIs, authentication, or streaming contracts.
- **NG-5**: It will not relax JSON schema, grammar, or safety constraints to gain speed.

### Constraints

- **C-1**: The project is Rust + optional `llama-cpp-2` (default feature `llama`); CI and contributors must be able to build with `--no-default-features` for fast agent/CI runs.
- **C-2**: Any new dependencies must be justified (Section 4) and pass existing audit/review practices.
- **C-3**: macOS and Linux must remain supported at least at current CI levels unless explicitly scoped.

### Scope Check

Single cohesive initiative: local inference **warmup and per-session latency** for the interactive REPL and engine layering. If GPU-specific tuning becomes large, it may be split into a follow-up PRD.

## 3. User Stories and Requirements

### User Personas

- **P1 — Interactive shell user**: Runs `clai interactive` in local mode, expects the session to feel “ready” quickly and each line to be responsive.
- **P2 — Scripter / integrator**: Invokes `clai ask` from scripts; cares about one-shot latency and may run many invocations in a loop; needs clear documentation of cost model.
- **P3 — Maintainer / contributor**: Needs predictable CI times and behavior across `llama` and non-`llama` builds.

### User Stories

- **US-1 (P1, P0)**: As an interactive user in local mode, I want the session to become ready for inference without waiting until I type the first line so that I do not pay an unexpected multi-second (or longer) delay on my first request.
- **US-2 (P1, P0)**: As an interactive user, I want each subsequent line in the same session to avoid reloading the full GGUF from disk so that the REPL stays fast after startup.
- **US-3 (P1, P0)**: As an interactive user, I want steady-state completion latency to be no worse than today after optimizations, and measurably better where redundant work is removed (see FR/NFR).
- **US-4 (P2, P1)**: As a scripter, I want documentation that states whether `clai ask` loads the model on every run and what alternatives exist (e.g. use interactive, or a future daemon), so that I can choose an appropriate pattern.
- **US-5 (P3, P0)**: As a maintainer, I want automated checks to still pass for `--no-default-features` builds so that development and CI remain reliable.

**Acceptance criteria**

| ID | Story | Criteria | Priority |
|----|--------|----------|----------|
| AC-1 | US-1 | In local interactive mode, there is a defined “ready” or “warming” state; when ready, the first user line does not trigger **initial** GGUF load on the hot path, or the user saw explicit progress during startup. | P0 |
| AC-2 | US-2 | In one interactive session, the GGUF file is not read end-to-end from disk for every line; at most one load per session (excluding explicit `reload`). | P0 |
| AC-3 | US-3 | Second and later lines show improved or equal latency vs baseline; regressions are rejected unless documented as unavoidable trade-off. | P0 |
| AC-4 | US-4 | README or in-app help updated to describe one-shot vs session behavior. | P1 |
| AC-5 | US-5 | `cargo test --no-default-features` and clippy for that configuration remain green. | P0 |

### Functional Requirements

- **FR-1**: The system must preserve the existing behavior: interactive sessions support local and cloud selection according to current config and flags; local path resolution errors remain user-visible and non-fatal to the loop where applicable.
- **FR-2**: The system must continue to use a single `LocalLlamaSession` (or equivalent) for the lifetime of an interactive local session, reloading from disk only on explicit `reload` or first successful load, consistent with current semantics unless superseded by a documented migration.
- **FR-3**: If **eager** or **background** model warmup is added, the system must provide a way to **disable** it (config or env) for low-memory or scripting environments, defaulting to the safer option if defaults conflict.
- **FR-4**: The system must not block session exit or stdin handling indefinitely on background warmup; failure to warm must surface as a clear error or fallback to lazy load.
- **FR-5**: The system must log or user-display (when verbose) sufficient detail to distinguish “loading weights”, “initializing context”, and “generating” for support and benchmarks, without leaking secrets.

### Non-Functional Requirements

- **NFR-1 (Performance)**: For interactive local use, after any chosen warmup completes, **p95** time from line submit to first streamed token (when streaming is enabled) must be **no worse than baseline p95** on the same model and machine unless justified in Appendix with a user-facing opt-in.
- **NFR-2 (Performance)**: First-line **additional** cost attributable solely to GGUF disk load in interactive mode must be **not present on the critical path** after the solution, or must be **overlapped** with session start such that the user wait from “session opened” to “first token” is reduced vs current “first line includes load”.
- **NFR-3 (Resource)**: Peak RSS during interactive use must not exceed **125% of baseline** without opt-in, per **SC-4**.
- **NFR-4 (Reliability)**: Warmup failures must not crash the process; the session must fall back to existing lazy load or exit with a non-zero code and message, as appropriate.
- **NFR-5 (Compatibility)**: Default release builds with `llama` feature must behave per FR and NFR; non-`llama` builds must remain unchanged in user-visible behavior (no spurious local load attempts).

## 4. Solution Design

### Approach

Current implementation (as of discovery) **reuses the loaded `LlamaModel` across interactive turns** by storing `local_session: Option<LocalLlamaSession>`; the first user completion opens the session and later turns reuse it. The **model is not reloaded on every command** in interactive mode. Gaps: **lazy first load** (no work at `run_interactive_session` start), and per-completion work inside `complete_with_loaded_model` that may **allocate a new context** each time—worth profiling as a post–weight-load cost center.

The solution direction: (1) **optional eager or background warmup** at session start for local mode; (2) **profile and reduce** redundant per-request initialization within a session subject to llama.cpp safety and context lifetime rules; (3) **document** the cost model for `clai ask` vs interactive.

### Key Design Decisions

| Decision | Context | Options | Rationale | Trade-offs |
|----------|---------|---------|------------|------------|
| D-1: Eager vs background warmup | First-line delay | Block until loaded; or background with spinner; or keep lazy | Blocking is simplest; background improves perceived responsiveness on slow disks | Background adds complexity and state machine edge cases |
| D-2: Context reuse | Per-turn latency in session | Reuse a long-lived context vs create per completion | TBD by profiling; API may constrain reuse | Reuse may increase memory; creation may waste CPU |
| D-3: Config surface | User control | New config key, env only, or CLI flag | Env/config keeps CI stable; CLI for power users | More documentation burden |

### Architecture Overview

- **Interactive loop** (`run_interactive_session`): extend lifecycle to include an optional **warmup phase** before the readline loop, or immediately after `print_session_start`, for local + `llama` only.
- **Engine** (`LocalLlamaSession` / `complete_with_loaded_model`): separate **model lifetime** (already session-scoped) from **inference context lifetime**; optimize the latter with measurements.
- **One-shot** (`complete_local_with`): out of scope for automatic cross-invocation reuse (NG-2, NG-3) unless a later initiative adds a server.

Data flow: unchanged at the user level: stdin → local/cloud completion → parse JSON proposal → policy → optional execute.

### Modular Design Principles

- Prefer changes localized to `session.rs` and `engine/llama.rs` with thin interfaces.
- Reuse existing env patterns (`CLAI_*`) before adding a parallel system.

### Security Considerations

- No change to trust boundaries: same local GGUF path resolution and no new network for local mode.
- Verbose or debug output must not log API keys (cloud path unchanged).
- If background threads are used, avoid data races on shared model state; follow llama.cpp threading expectations.

## 5. Alternatives Considered

- **A-1: Keep lazy load only, document** — Pros: zero code risk. Cons: first-line pain remains; fails primary user expectation.
- **A-2: Long-lived `clai serve` process** — Pros: one load for many clients. Cons: new operational model, scope creep (rejected for NG-2 in v1).
- **A-3: mmap / OS cache only** — Pros: no app changes. Cons: may not remove CPU-side init; insufficient alone.
- **Verdict**: Combine **A-1** (documentation) with **eager/background warmup** and **measured** engine optimizations for a balanced first deliverable.

## 6. Implementation Plan

### Phased Rollout

- **Phase 1 (MVP)**: Document current behavior; add optional eager or blocking warmup in interactive local mode; measure first-line and second-line latencies; gate behind config/env.
- **Phase 2**: Profile `complete` path; implement safe reuse or pooling of work items where validated; tune defaults with memory caps.
- **Phase 3 (Future)**: Optional local daemon or socket-based inference if product needs cross-process reuse.

### Tech Stack Alignment

- Rust, `cargo`, `llama-cpp-2` as in `Cargo.toml`. No new package manager.
- Quality gates: match `.github/workflows/ci.yml` and local full build practice.

### Migration and Compatibility

- Defaults must preserve current lazy behavior for users who rely on fast session **startup** (no block) unless we choose opt-in eager load only—**open for product decision** (see Section 9).

## 7. Testing Strategy

### Testing Levels

- **Unit / integration**: Extend or add tests around session setup where testable without loading multi-GB models (e.g. feature-gated, or with tiny fixtures if the project adds them). Validate config parsing for new flags.
- **Manual / benchmark**: Document a repeatable script to measure time-to-first-token (second line vs first line) for a given GGUF; store baseline numbers in Appendix or internal notes.
- **E2E**: Optional smoke: interactive in CI remains impractical for full model; keep CI to existing non-interactive tests.

### Validation Approach

- Compare metrics before/after on the same device and model path.
- Ensure streaming and non-streaming paths both work.

### Quality Gates

All commands below must pass for changes merged under this PRD (align with [`.github/workflows/ci.yml`](.github/workflows/ci.yml) and local release practice):

- **QG-1**: `cargo fmt --check` — formatting matches `rustfmt` for the project.
- **QG-2**: `cargo test --no-default-features --locked` — test suite passes as in CI.
- **QG-3**: `cargo clippy --no-default-features --locked -- -D warnings` — no clippy warnings, as in CI.
- **QG-4**: `cargo build --locked` — default features build (includes `llama` when enabled), matching `build-full` job expectations for release-capable artifacts.
- **QG-5**: Code review completed with explicit check for memory and threading notes if warmup/background is introduced.

*Note: `build-full` is macOS-only in CI; developers should run `cargo build --locked` locally on their target platform before merge.*

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| R-1: Context reuse introduces UB or Heisenbugs in llama.cpp bindings | Med | High | Favor small, well-reviewed steps; add tests; consult upstream patterns |
| R-2: Eager load increases OOM on small machines | Med | Med | Off-by-default or memory-aware defaults; clear env to disable |
| R-3: Perceived “hang” on session start if blocking load | Med | Med | Progress messages, optional non-blocking path |
| R-4: Scope creep into daemon / IPC | Med | Med | Enforce non-goals; separate PRD |
| R-5: CI flakiness if tests touch GPU or huge files | Low | Med | Keep CI on `--no-default-features` for stability |

## 9. Open Questions

1. **OQ-1 (Product)**: Should **eager warmup** be default-on, default-off, or autodetected (e.g. only when TTY and model size below threshold)? *Owner: product or maintainer.* *Impact: affects SC-1 and user surprise.* *Default proposed: off until benchmarked, with README recommendation.*
2. **OQ-2 (Engineering)**: Is **long-lived `LlamaContext`** across completions in-session supported and beneficial with our `llama-cpp-2` version, or is per-request context creation required? *Owner: implementer with profiling.* *Impact: drives Phase 2 effort.*
3. **OQ-3 (UX)**: Should the REPL show a **spinner** or “warming model…” during background load? *Owner: UX/maintainer.* *Impact: perceived latency.*
4. **OQ-4 (Metrics)**: What is the official **baseline** commit and hardware profile for before/after numbers? *Owner: team.* *Impact: success criteria verifiability.*

## 10. Appendix: Discovery notes (current implementation, pre-change)

- **Interactive**: [`run_interactive_session`](../../src/session.rs) initializes `local_session` to `None` and **loads the model on the first** successful local completion path (or on `reload` if not yet loaded). **Subsequent** turns call `local_session.as_mut().complete(...)` only — no repeat `open` of the file for each line.
- **Engine**: [`LocalLlamaSession::open`](../../src/engine/llama.rs) loads GGUF once per session. [`complete_with_loaded_model`](../../src/engine/llama.rs) runs the full **decode pipeline** and constructs **context** inside that function; profiling should verify whether that implies **new context per call** and whether reuse is possible.
- **One-shot `ask`**: [`complete_local_with`](../../src/engine/llama.rs) calls `LocalLlamaSession::open` then a single `complete` — full load each **process** invocation.
- **Cloud**: no local model load; different latency profile.

*This appendix documents discovery evidence and is not a specification of future code shape.*
