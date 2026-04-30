# PRD: Adaptive script execution and runtime tool awareness

## 1. Executive Summary

**Problem statement:** `clai` always asks the model to emit a single JSON [`CommandProposal`](../../../src/schema.rs) (argv + optional `cwd`). The system prompt today exposes only broad host facts (OS, arch, cwd, shell family, path separator) via [`build_system_prompt`](../../../src/main.rs). For tasks that are clearer, shorter, and faster as a small program (Python, Node, Ruby, etc.), a one-shot shell pipeline or long `sh -c` string is often worse: harder to read, more error-prone, and a poor fit for the model. Users also get no first-class signal about which interpreters are actually on `PATH`, so the model may guess wrong or pick heavier approaches than necessary.

**Proposed solution:** Enrich the context passed to the model with **structured detection of common host tooling** (e.g. whether `python3`, `node`, `ruby`—and similar—resolve on the host, without requiring a full system inventory). Complement that with **clear instructions** so capable models prefer a **short script** (or `interpreter -c` / file invocation) when it fits the user request better than a fragile shell command. When a multi-line script must live on disk, the **runtime** must create it under a **managed temporary contract**, run the proposed argv, and **clean up** those artifacts in all exit paths (success, failure, timeout, user abort).

**Success criteria**

| ID | Criterion | Target |
|----|-----------|--------|
| SC-1 | Ephemeral script files created by the managed path are removed after the child exits | 100% in automated tests covering success, non-zero exit, executor timeout, and user cancel where applicable |
| SC-2 | Tooling summary is observable for support/debug (via existing `doctor` or equivalent user-facing surface) | When the feature is enabled, at least the presence (or absence) of each detected category is listed |
| SC-3 | No regression in security posture: cwd jail and policy rules | Existing policy tests pass; new tests cover any expanded allowlist/artifact paths |
| SC-4 | Documented benchmark or golden prompts (see §9) | At least 5 scenarios where script-style answers are expected, used for manual or CI checks |

## 2. Goals and Non-Goals

### Goals

1. **G-1:** The model can make **better-informed** choices between “shell one-liner / pipeline” and “small program in an available runtime,” using real host data rather than guesswork.
2. **G-2:** Users get **reliable** execution when a script file is needed: **no leftover** temp files from normal `clai` operation under the managed contract.
3. **G-3:** Behavior remains **safe and auditable**: policy, confirmation flows, and execution modes (`direct` / `docker` / `bwrap`) stay coherent; new surfaces do not bypass policy.
4. **G-4:** The feature is **shippable in phases** (see §6): early value from prompt + tool detection without necessarily shipping file-backed scripts in the first release.

### Non-Goals

- **NG-1:** The project will **not** install, upgrade, or manage language runtimes for the user.
- **NG-2:** The project will **not** build a full package-manager or project-detection system (e.g. auto-detect `package.json` vs `pyproject.toml` and wire installs) in this initiative.
- **NG-3:** The project will **not** require **shell** as the default way to run multi-line logic; existing rules around `needs_shell` and executor behavior remain; expanding shell reliance is out of scope unless explicitly approved in a follow-up.
- **NG-4:** The project will **not** guarantee **model** creativity or correctness for every task (model variance remains); the scope is **enabling** better inputs and a **safe** execution path for script artifacts.
- **NG-5:** The project will **not** add network calls solely to “phone home” what is installed; detection is **local** to the machine running `clai`.

### Constraints

- **C-1:** Primary codebase is **Rust** (`clai`); changes must align with [Cargo.toml](../../../Cargo.toml) and existing modules (`host_context`, `session`, `policy`, `executor`, schema/prompt build path).
- **C-2:** **CI** today runs `cargo fmt --check`, `cargo test --no-default-features --locked`, `cargo clippy` with warnings denied, and full-feature `cargo build` (see [.github/workflows/ci.yml](../../../.github/workflows/ci.yml)); all must remain green.
- **C-3:** **Policy** allowlists and strict mode must remain meaningful; users who rely on strict allowlists must not get silent execution of unapproved binaries.
- **C-4:** **Cross-platform** behavior must be explicit: Unix and Windows may differ in which tools are probed and how temp paths are handled (see §9).

### Scope check

This PRD is **one** cohesive initiative: **smarter context + optional ephemeral script materialization + cleanup**. It does not bundle unrelated inference or UI work.

## 3. User Stories and Requirements

### User personas

- **P-1 – Developer on the host:** Uses `clai ask` or interactive mode daily; wants fast, maintainable proposed commands and minimal surprises on disk.
- **P-2 – Automation user:** Runs `clai` in CI or scripts; needs deterministic cleanup and clear logs about what ran.
- **P-3 – Security-conscious user:** Relies on policy, dry-run, and confirmations; wants new behavior to **not** weaken guarantees.

### User stories

**US-1 (P-1, P-2):** As a developer, I want `clai` to **know** which common runtimes exist on my machine so that proposed commands use **runnable** interpreters.  
- **Acceptance criteria:** (A) Given a host with `python3` on `PATH`, the context the model sees indicates that a Python-style invocation is available. (B) Given a host without `node`, the context does not claim Node is available.  
- **Priority:** P0  

**US-2 (P-1):** As a user, I want the assistant to **prefer a short script** when that is **clearer or more robust** than a long shell command, so that I can read and trust what will run.  
- **Acceptance criteria:** (A) System instructions describe when to prefer script-style answers. (B) The model is not **forced** to use scripts for trivial single-invocation tasks.  
- **Priority:** P0  

**US-3 (P-1, P-2):** As a user, I want any **temporary script file** that `clai` creates to be **removed automatically** after execution, so my workspace and tmp directories are not littered.  
- **Acceptance criteria:** (A) On successful child exit, temp artifacts under the managed contract are removed. (B) On non-zero exit, timeout, or cancel, the same. (C) A test or harness asserts the temp directory is clean afterward.  
- **Priority:** P0  

**US-4 (P-3):** As a security-conscious user, I want **policy** to apply to whatever runs (including an interpreter running a temp file), so that allowlists and confirmations still work.  
- **Acceptance criteria:** (A) No new execution path bypasses `PolicyEngine`. (B) Documentation states how temp script paths interact with cwd jail.  
- **Priority:** P0  

**US-5 (P-1):** As a user, I want to **see** what will run (existing pre-run / preview behavior) with **clear** indication when a temp script is involved, so I can approve confidently.  
- **Acceptance criteria:** Pre-run output distinguishes “running interpreter on managed temp file” from a simple argv list when the former applies.  
- **Priority:** P1  

### Functional requirements

**FR-1:** The system must discover **at minimum** whether each configured category of common tooling is **invokable** on the host (e.g. via `which`-style or equivalent resolution), and must expose that summary to the model through the same prompt pipeline as `build_system_prompt` (extended or adjacent).  

**FR-2:** If tool discovery measurably increases startup latency beyond the budget in **NFR-1**, discovery must be **off by default** or **opt-out via configuration** until optimized (see also **NFR-2**).  

**FR-3:** The system must add **instructional** text to the system prompt so the model, when a suitable runtime is available, may prefer **script-style** command lines over fragile shell pipelines for tasks involving non-trivial logic, data transforms, or readability—without breaking the rule that the model’s reply is **only** the JSON object (no markdown or extra prose in the output).  

**FR-4:** The instructional text must state that **trivial** one-shot commands may remain a single `program` + `args` when that is the clearest fit.  

**FR-5:** The system must support a **managed ephemeral script** path where script bytes are **written** only to a **designated** temporary area, **executed** via the normal `CommandProposal` → policy → executor pipeline, and **deleted** afterward in every documented completion path.  

**FR-6:** The user must be able to **disable** file-backed script materialization via configuration or flag (default: **TBD in §9**).  

**FR-7:** The system must ensure **policy** evaluates the final executable (`program` + `args`) used to launch the process, including when `args` reference a path under the managed temp contract.  

**FR-8:** **Cwd jail** checks must remain **enforced** for the proposal’s `cwd` field when temp paths are used.  

**FR-9:** The system must log or surface (at least in `verbose` / diagnostic modes) the **count** and **location pattern** of managed temp artifacts for a run, without dumping full script contents at default verbosity. **Verbose** behavior (path vs short hash) is **TBD in §9**.  

**FR-10:** The system must keep **cloud** and **local** inference paths **consistent** in what host context is sent; if behavior differs, documentation must state the rule. **Default handling in §9.**  

**FR-11:** The system must not expand **shell injection** risk: the model’s output remains structured JSON; the runtime does not `eval` arbitrary shell to build scripts.  

**FR-12:** Script bytes are **data files** opened by the **chosen interpreter** (or equivalent) per the proposal, not shell snippets, unless `needs_shell` policy allows shell execution.  

**FR-13:** `clai doctor` (or a dedicated subcommand) must list **which tool categories were detected** when the user requests diagnostic output, to satisfy **SC-2**.  

### Non-functional requirements

**NFR-1:** For interactive `clai` sessions, the additional latency of tool detection on startup must stay **under 500 ms at p95** on a typical developer laptop when **five or fewer** categories are probed, or detection must be **cached** for the process lifetime.  

**NFR-2:** Tool detection must be **disable-able** in configuration for environments where `PATH` probes are forbidden or slow.  

**NFR-3:** Ephemeral files must be created with **user-only** permissions where the OS supports it; path must not be world-writable.  

**NFR-4:** Under executor **timeout**, any child must be stopped per existing `executor` behavior, and **cleanup** of managed temp files must still run (best-effort: if cleanup fails, error must be surfaced in verbose/diagnostic).  

**NFR-5:** The feature must be covered by **automated tests** at the level appropriate to the project: unit tests for path helpers and cleanup; integration tests for “temp dir empty after run” on at least one OS in CI, or cross-platform with conditional skips documented.  

## 4. Solution Design

### Approach

Split the problem into **(A) observability of the host** and **(B) contract for ephemeral scripts**. (A) is low risk: it only changes what the model “knows.” (B) is higher risk: it touches filesystem, policy, and lifecycle. Ship (A) early; gate (B) on design review and tests.

**Key design decisions** (concrete field names and APIs are left to implementation; the PRD constrains **behavior**):

| Decision | Context | Options considered | Rationale | Trade-offs |
|----------|---------|--------------------|-----------|------------|
| D-1: Where tool info lives | Today [`HostContext`](../../../src/host_context.rs) holds shell and OS | Extend `HostContext` vs new `RuntimeTooling` struct | A dedicated struct keeps JSON stable and testable; `HostContext::to_prompt_json` may merge or embed | Slightly more types to maintain |
| D-2: How the model provides script text | New JSON field vs `stdin` | Only argv today | New field or a side-channel is needed for file-backed scripts; `python -c` may avoid files | Schema migration + grammar updates vs simpler `-c` only in phase 1 |
| D-3: Temp root | System temp vs clai data dir | Use OS temp with unique subdir per run; optional config override | OS temp is expected for short-lived; data dir for audit — optional | System temp may have OS-specific policies |
| D-4: Model capability gating | Ignore vs “strong models only” | Config flag or prompt tier | Avoid misleading small local models; optional strictness | More config surface |

### Architecture overview

- **Detection layer:** On ask/session start (or first prompt), run bounded probes; cache in memory for the process. Feed merged JSON into the system string alongside `HostContext`.
- **Prompt layer:** Extend instructions: prefer script when **appropriate**; list available runtimes; remind that output is **only** JSON.
- **Materialization layer (optional phase):** After parsing `CommandProposal`, if the proposal implies script file content (per chosen design in §9), write bytes, substitute **absolute** or **jailed** path into `args` before policy, or resolve path after policy for the same `program`—**must** preserve policy semantics (see **FR-7** and **FR-8**).
- **Execution:** Unchanged `executor::run_proposal` entry; **cleanup** in a `scopeguard`-style or `defer` after child wait in `cmd_ask` / session loop.

**New dependencies:** Prefer **no** new crates unless justified (e.g. `scopeguard` or `tempfile` are already a dev-dependency; promote or use std only). Justify in implementation if a new crate is required.

### Security considerations

- **Authentication:** N/A (local CLI).
- **Authorization / policy:** All launches remain subject to `PolicyEngine`; `strict_allowlist` must either include proposed interpreters or the feature must document required allowlist entries.
- **Data protection:** Temp scripts may contain user-requested code; do not log full content at info level; optional verbose redaction.
- **Input validation:** Script body from the model is **untrusted**; path placement and permissions reduce risk; the user still confirms per policy.
- **Audit:** Verbose and dry-run should show **intent** and **interpreter** clearly.

## 5. Alternatives Considered

| Alternative | Pros | Cons | Verdict |
|-------------|------|------|---------|
| **A: Prompt-only (tell model to use `python -c` / `node -e`)** | No temp files, minimal code | Long `-c` strings are awkward; Windows quoting differs | **Adopt** as Phase 1 baseline; combine with tool detection |
| **B: Always use shell to write temp files** | Flexible | Reintroduces shell dependency and injection surface | **Reject** for core path; conflicts with `needs_shell` posture |
| **C: Let model output raw script in `reason` or another text field and parse** | No schema change to argv | Brittle, mixes concerns | **Reject** as primary; might inform debug only |
| **D: Docker-only script execution** | Isolated | Does not help default `direct` users | **Defer**; optional later for `ExecutionMode::Docker` |

## 6. Implementation Plan

### Phased rollout

- **Phase 1 (MVP):** **FR-1, FR-2, FR-3, FR-4, FR-13**, **NFR-1–NFR-2** — tool detection + prompt updates + `doctor` visibility. No file-backed scripts. Validate with golden prompts (manual or CI). **Independently shippable.**
- **Phase 2:** **FR-5–FR-9, FR-11, FR-12**, **NFR-3–NFR-5**, **US-5** — managed ephemeral file lifecycle, cleanup hooks, pre-run display updates, policy tests. **Independently shippable** behind a config flag if needed.
- **Phase 3 (optional):** **FR-10** consistency hardening, cloud path documentation, and performance micro-optimizations (caching across sessions if demanded).

### Tech stack alignment

- **Rust** project; use `cargo` workflows; follow existing `serde` and module patterns.
- Reuse `tempfile` if temp dirs are needed (already in `dev-dependencies`—evaluate `promote` vs std-only for release).

### Migration and compatibility

- Config schema: add a `[tooling]` or `ask` subsection with `detect_runtimes = true` and `ephemeral_scripts = false` (defaults **TBD §9**).
- Existing users: **no behavior change** when detection is off; when on, only prompt and `doctor` change until Phase 2.

## 7. Testing Strategy

### Testing levels

- **Unit:** Mock `PATH` / probe results; JSON merge for prompt; temp path creation and `Drop`/cleanup.
- **Integration:** End-to-end `clai ask` with `--print-only` to verify prompt includes tooling block (golden string fragments); cleanup tests with a real small interpreter if CI image has it.
- **E2E:** Optional: one scenario in existing smoke harness if the repo adds it later; not required in PRD if not present.

### Quality gates (must all pass for merge)

**QG-1:** `cargo fmt --check` — formatting matches Rust style.  
**QG-2:** `cargo test --no-default-features --locked` — all tests pass on slim feature set.  
**QG-3:** `cargo clippy --no-default-features --locked -- -D warnings` — no clippy warnings.  
**QG-4:** `cargo build --locked` — default features build (matches CI `build-full`).  
**QG-5:** Code review with explicit check of policy + temp cleanup paths.  

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `PATH` probing is slow or blocked on enterprise hosts | Med | Med | NFR-2 off-switch; cache; cap probes; document |
| Model emits invalid paths or wrong interpreter | Med | Med | Keep grammar/schema validation; user confirmation; strong print-only / dry-run story |
| Temp file leaks on panic/kill | Low | High | `scopeguard` / RAII; tests for timeout path |
| Policy allowlist blocks `python3` on strict setups | Med | Med | Document allowlist examples; error messages that mention tooling |
| Windows vs Unix divergence | Med | Med | Conditional tests; explicit non-goals for unsupported tools |

## 9. Open Questions

1. **Schema vs `-c` only in Phase 2:** Should file-backed scripts require a **new optional field** in the JSON output (e.g. script body and encoding), or only support `interpreter` + file path the runtime fills? **Owner:** eng lead. **Impact:** Affects grammar, cloud structured outputs, and API compatibility. **Default:** Phase 1 ships prompt + detection only; Phase 2 chooses schema extension after spike.
2. **Default for `ephemeral_scripts`:** On or off by default? **Owner:** product + security. **Impact:** User surprise vs convenience. **Default:** **off** until Phase 2 is stable.
3. **Model capability gating:** Use **config** only, or detect local GGUF “size tier”? **Owner:** eng. **Impact:** Prompt complexity. **Default:** single global config flag `prefer_scripts_when_available = true` with no model-tier logic initially.
4. **Cloud mode:** Send **same** synthetic tool snapshot as local, or omit detection (cloud model might hallucinate less if we omit)? **Owner:** eng. **Impact:** Consistency (**FR-10**). **Default:** send **cached local snapshot** from the machine running the CLI, clearly labeled, so the cloud model matches reality.
5. **Benchmark set:** The five+ golden prompts (SC-4) — who curates, and in-repo vs private? **Owner:** team. **Default:** add `tests/golden` or `assets/ask-prompts/README` with static examples, **no** CI flakiness from local LLM unless separately gated.
6. **Discovery default (FR-1 vs FR-2):** The implementation must either (a) meet **NFR-1** with caching or (b) ship with tool discovery **off by default** until (a) holds. **Default strategy:** (a) with caching, else (b).

## 10. Appendix

### Glossary

- **Command proposal:** Structured JSON with `program`, `args`, optional `cwd`, `reason`, `needs_shell`, `confidence` — see [`src/schema.rs`](../../../src/schema.rs).
- **Managed temp contract:** Directory and naming convention owned by `clai` for Phase 2 script materialization, with guaranteed cleanup.
- **Tool category:** e.g. Python, Node, Ruby — a probe target, not necessarily a full version matrix.

### Related artifacts

- Archived plan: [plans/archive/native-shell-execution-ux/prd.md](../native-shell-execution-ux/prd.md) (shell UX; different scope from script-first with argv).
- Executor: [src/executor.rs](../../../src/executor.rs)
