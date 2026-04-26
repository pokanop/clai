<!-- PRD: plans/native-shell-execution-ux/prd.md -->
<!-- Generated: 2026-04-26 -->
<!-- Last Updated: 2026-04-26 -->

# Tasks: Native shell execution UX for `clai ask`

> Implements the PRD’s default “run like the shell” path: terminal-connected I/O and exit codes in direct mode, minimal default scaffolding, opt-in verbose/structured output, non-TTY tests in CI, and documentation/migration notes—without weakening policy or adding Phase 1 shell plugins.

## 1. Overview

### Project Summary

Today `clai ask` always pretty-prints the full proposal as JSON and runs the child with piped stdout/stderr, then re-labels output as `status` / `stdout` / `stderr`. The process exit code does not reflect the child’s outcome. This initiative refactors the executor and `ask` presentation so the default path inherits the user’s terminal for direct execution when appropriate, propagates the child exit code, keeps at most one clean pre-exec line, and moves structured diagnostics behind an explicit opt-in—while preserving policy, confirmation, and non-direct (Docker/bwrap) capture with clearer attribution.

### Scope Reference

- PRD: [plans/native-shell-execution-ux/prd.md](./prd.md)
- **Phase 1 (MVP):** Default human mode, TTY-connected I/O (direct), exit propagation, verbose opt-in, non-TTY tests, migration note, no repo-hosted shell snippets.
- **Phase 2:** Extra flags (force capture, suppress preview), docs tightening, expanded automated edge-case coverage.
- **Phase 3 (optional / future):** Shell snippets, `needs_shell` story, clipboard, CI PTY—out of this task list’s committed scope (see Future Considerations).

### Open questions affecting planning

None. Resolved decisions are recorded in PRD Section 9.

### Task Statistics

| Metric        | Count |
| ------------- | ----- |
| Total Tasks   | 19    |
| Completed     | 19    |
| In Progress   | 0     |
| Blocked       | 0     |
| Not Started   | 0     |

## 2. Phase sections

## Phase 1: MVP — default shell-native direct path (13/13 tasks complete)

> Delivers human-default `ask` UX, inherited stdio for direct+TTY, child exit code propagation, opt-in verbose/structured output, non-direct attribution, automated non-TTY coverage, migration + manual TTY checklist, and no first-party shell paste-in docs.
>
> **Goal:** Shipping Phase 1 changes the default terminal experience and exit codes in a documented, breaking way; CI proves exit/output routing on the direct path without PTY tests.

### Execution layer

- [x] **1.1 Introduce stream strategy and display-mode selection** `[P0]` `[M]`
  - **Depends on**: None
  - **Requirements**: FR-1, FR-2, NFR-1, NFR-3
  - **Acceptance Criteria**:
    - [x] A single, testable decision function (or small module) determines when the direct path uses inherited stdio vs piped capture, using at least: `ExecutionMode::Direct`, human vs verbose output intent, and whether stdout/stderr (and stdin if applicable) should attach to the user TTY per PRD (e.g. via `std::io::IsTerminal` / project conventions).
    - [x] Verbose/machine-oriented or non-direct profiles always select capture where the PRD requires it.
    - [x] Unit tests cover mode selection matrix (direct+human+non-TTY → capture; direct+human+TTY → inherit; verbose → capture; docker/bwrap → capture) without requiring a live LLM.
  - **Notes**: Keep policy evaluation and argv construction out of this pure “how we run / connect streams” decision. Align naming with existing `ExecutionConfig` / `ExecutionMode` in `src/config`.
  - **Completed**: 2026-04-26. Added `src/stream_strategy.rs` with `OutputIntent`, `UserTerminalContext`, `StreamStrategy`, `select_stream_strategy`, and `current_user_terminal_context`; unit tests for the full matrix. Executor will consume this in task 1.2.

- [x] **1.2 Refactor proposal execution to support inherited stdio** `[P0]` `[L]`
  - **Depends on**: Task 1.1
  - **Requirements**: FR-2, FR-3, FR-6, NFR-3
  - **Acceptance Criteria**:
    - [x] `executor` exposes a clear API (either extended `run_proposal` or adjacent entry points) that runs the child with inherited stdin/stdout/stderr when Task 1.1 selects inherit, and with piped capture otherwise.
    - [x] Existing timeout behavior is preserved for both paths: a hung child is terminated and the outcome distinguishes timeout from normal exit (consistent with current `RunOutcome` semantics or a deliberate, documented evolution).
    - [x] Non-direct modes (`Docker`, `Bwrap`) remain capture-first; no host TTY forwarding required in Phase 1 per PRD §9.
    - [x] Unix direct-mode `pre_exec` / process-group behavior is preserved or intentionally updated with tests/docs if behavior changes.
  - **Notes**: Today `run_proposal` always sets `stdin(null)`, piped stdout/stderr (`src/executor.rs`). Windows job-object / breakaway logic must remain correct for the direct inherited path.
  - **Completed**: 2026-04-26. `run_proposal` takes `StreamStrategy`; inherit uses `Stdio::inherit()` (direct only), capture unchanged; `finish_child` returns empty strings on inherit; module docs; unit tests for capture, inherit, and reject inherit+docker. `ask` uses `select_stream_strategy` + `current_user_terminal_context()`.

- [x] **1.3 Map child exit status to process exit codes** `[P0]` `[M]`
  - **Depends on**: Task 1.2
  - **Requirements**: FR-3, US-2, SC-1, NFR-1
  - **Acceptance Criteria**:
    - [x] For completed child runs (no policy block, no user abort, no pre-exec failure), `clai` exits with the child’s exit code on platforms where `ExitStatus::code()` is defined; signal-terminated cases are documented (README or module docs) per platform conventions.
    - [x] Timeout and kill paths produce a non-zero, documented exit code for `ask`.
    - [x] Unit tests cover exit-code mapping helpers where platform allows deterministic simulation.
  - **Notes**: `main` currently exits `1` only on `Result::Err` (`src/main.rs`); `ask` will need explicit `std::process::exit` or a structured return path for child status.
  - **Completed**: 2026-04-26. Added `ask_exit` with `clai_ask_process_exit_for_child`, `CLAI_ASK_TIMEOUT_EXIT` (124), `RunOutcome::clai_ask_process_exit`; `cmd_ask` calls `std::process::exit` after a successful `run_proposal`. README + `ask_exit` module docs. Tests: `maps_zero_and_nonzero`, `run_proposal_timeout_sets_process_exit_124` (Unix).

### CLI presentation (`ask`)

- [x] **1.4 Add explicit opt-in for verbose / structured diagnostics** `[P0]` `[M]`
  - **Depends on**: None (can parallelize with 1.1–1.2 but must integrate before 1.5)
  - **Requirements**: FR-1, US-3, SC-3
  - **Acceptance Criteria**:
    - [x] Users can opt in (CLI flag and/or config-driven behavior per PRD FR-1) to see full proposal JSON and structured execution details similar to today’s pretty-printed proposal + `status`/`stdout`/`stderr` blocks.
    - [x] Default (non–opt-in) path does not dump full structured proposal before execution completes.
    - [x] `--print-only` semantics remain coherent: proposal only, no execution (may fold into “verbose family” or stay distinct with clear help text).
  - **Notes**: Prefer extending existing `Ask` flags (`src/main.rs`) over proliferating overlapping flags; document the contract in `--help`.
  - **Completed**: 2026-04-26. `--verbose` / `-v` + `CLAI_ASK_VERBOSE` + `ask_verbose` in config; `verbose_ask` sets `OutputIntent::Verbose` and pre-exec `Proposed:` JSON. Default human path skips that line; `print_only` still prints proposal then exits. Post-exec report unchanged (task 1.5 will minimize default).

- [x] **1.5 Default human output: minimal scaffolding + optional one-line preview** `[P0]` `[L]`
  - **Depends on**: Tasks 1.2, 1.4
  - **Requirements**: FR-1, US-1, SC-3, FR-5
  - **Acceptance Criteria**:
    - [x] Default human mode prints at most one clean line of pre-execution feedback (e.g. what will run), optional to suppress in non-interactive contexts if implementation chooses, per PRD §9 decision #1.
    - [x] After execution in direct+inherited mode, child output is not framed inside a generic `stdout:` / `stderr:` tool report by default.
    - [x] No secrets or policy-bypass instructions in new output; follow existing redaction/omission patterns (`FR-5`).
  - **Notes**: Replace `println!("Proposed: {}", serde_json::to_string_pretty(&proposal)?);` as default behavior in `cmd_ask`.
  - **Completed**: 2026-04-26. Human default: one `Run: <argv>` line when `stdout` is a TTY; post-exec unlabeled `print!`/`eprint!` for capture, none for inherit; verbose unchanged. Unit tests in `main` for preview helpers and FR-5 (no `reason` in line).

- [x] **1.6 Policy, confirmation, and dry-run exit semantics** `[P0]` `[M]`
  - **Depends on**: Tasks 1.3, 1.5
  - **Requirements**: FR-4, NFR-3, US-2
  - **Acceptance Criteria**:
    - [x] Policy-blocked runs exit with failure (non-zero) and message; no claim of successful execution.
    - [x] User declines confirmation at prompt: non-zero exit (or documented consistent “aborted” code) and message; not indistinguishable from successful command exit `0`.
    - [x] Dry-run and other non-execution paths do not report a fake successful child exit code.
  - **Notes**: Today `cmd_ask` returns `Ok(())` on abort and after printing dry-run (`src/main.rs`); reconcile with FR-4 and US-2 expectations.
  - **Completed**: 2026-04-26. `CLAI_ASK_USER_DECLINED_EXIT` (2) and `CLAI_ASK_DRY_RUN_EXIT` (3) in `ask_exit`; `process::exit` for decline/dry-run. Policy still `Err` → main exit 1. `--print-only` remains `Ok(0)`; README documents semantics.

### Non-direct attribution

- [x] **1.7 Summarize non-direct runs with program, cwd, and profile** `[P0]` `[M]`
  - **Depends on**: Task 1.5
  - **Requirements**: FR-6, US-1
  - **Acceptance Criteria**:
    - [x] For `Docker` and `Bwrap` profiles, user-visible output includes identifiable program, working directory, and execution profile before or alongside captured streams so output is attributable to the invoked command.
    - [x] Default human mode stays minimal; verbose mode may repeat or expand the same metadata for operators.
  - **Notes**: Builds on capture-first non-direct behavior per PRD.
  - **Completed**: 2026-04-26. `non_direct_context_one_line` (human: pre-TTY, or pre-streams if non-TTY) and `non_direct_context_verbose` before status block. Unit tests in `main` for direct/docker/bwrap.

### Testing and quality

- [x] **1.8 Unit tests: mode selection and exit/status helpers** `[P0]` `[M]`
  - **Depends on**: Tasks 1.1, 1.3
  - **Requirements**: US-4, NFR-1, QG-1
  - **Acceptance Criteria**:
    - [x] `#[cfg(test)]` modules cover stream-strategy selection and exit-code mapping logic with deterministic inputs.
    - [x] `cargo test --no-default-features --locked` passes locally for new tests.
  - **Notes**: Follow patterns in `src/policy.rs`, `src/schema.rs`, etc.
  - **Completed**: 2026-04-26. Extended `stream_strategy` tests (`all_streams_tty`, `OutputIntent` default), `ask_exit` Unix `SIGTERM` signal mapping, `executor::capture_failing_child_preserves_status_and_clai_ask_process_exit` (exit 7) linking mapping to `RunOutcome`.

- [x] **1.9 Integration tests: direct path exit propagation (non-TTY)** `[P0]` `[L]`
  - **Depends on**: Tasks 1.2, 1.3
  - **Requirements**: US-4, NFR-1, FR-3, SC-1
  - **Acceptance Criteria**:
    - [x] Automated tests invoke the executor (or a thin test-only harness) with trivial children (e.g. `true` / `false` / shell `exit N`) under **non-TTY** conditions and assert `clai`/`run_proposal` exit semantics match the child.
    - [x] At least one test covers verbose/capture path distinct from inherited path.
    - [x] No PTY-based assertions in CI for Phase 1 (per PRD and US-4).
  - **Notes**: If full `clai ask` E2E is impractical without a model, prefer library-level integration tests in `tests/` calling `executor::run_proposal` with synthetic `CommandProposal` values—consistent with PRD testing levels.
  - **Completed**: 2026-04-26. Added `tests/direct_path_exit_propagation.rs` (6 tests): `StreamStrategy::Capture` / `Inherit` / `OutputIntent::Verbose` + `select_stream_strategy` vs explicit capture. Fixed `clippy::io_other_error` in `ask_exit` helper. Use `cargo clippy --all-targets` for integration test crate.

- [x] **1.10 Performance guardrail: trivial child overhead** `[P1]` `[S]`
  - **Depends on**: Task 1.2
  - **Requirements**: NFR-2
  - **Acceptance Criteria**:
    - [x] A repeatable micro-benchmark or scripted timing check (documented in README or contributor docs) compares default direct capture vs inherited path for a no-op command; overhead stays within the PRD’s ~500ms order-of-magnitude guardrail on reference hardware, or deviations are explained.
  - **Notes**: Does not require Criterion unless maintainers prefer it; a simple `tests/` timing smoke or `cargo` example is enough if documented.
  - **Completed**: 2026-04-26. `tests/trivial_child_overhead.rs` (median 20 runs); README “Performance (NFR-2)” with command; notes on CI noise.

### Documentation and release notes

- [x] **1.11 Migration note and SC-2 manual verification checklist** `[P0]` `[M]`
  - **Depends on**: Tasks 1.5, 1.6, 1.7
  - **Requirements**: NFR-4, SC-2, US-4, FR-4
  - **Acceptance Criteria**:
    - [x] README and/or CHANGELOG describes breaking changes: default stdout layout and exit code behavior for script authors; **no** legacy flag/env to restore old behavior (PRD §9).
    - [x] Documented manual steps verify TTY behavior (color/pager representative command) on macOS and Linux for SC-2, since CI omits PTY in Phase 1.
    - [x] Signal / exit-status semantics briefly documented for script authors where platform-dependent.
  - **Notes**: PRD ties migration to README or CHANGELOG.
  - **Completed**: 2026-04-26. Added `CHANGELOG.md` (Unreleased breaking bullets); README *Migrating*, *Manual verification: TTY (SC-2)*, *Script authors (portability)* for signals/OS.

- [x] **1.12 Confirm Phase 1 doc scope: no first-party shell snippets** `[P2]` `[S]`
  - **Depends on**: Task 1.11
  - **Requirements**: US-5, PRD Non-Goals (shell plugins)
  - **Acceptance Criteria**:
    - [x] No new `zsh`/`fish`/`nu` paste-in example blocks or shell-plugin instructions are added to the repository in Phase 1 deliverables.
  - **Notes**: Future work belongs in a follow-up PRD (Phase 3). **Audit (2026-04-26):** no `zsh` / `fish` / `nushell` (or `nu`) fenced code blocks in the repository; no shell-plugin paste-in in `README.md` or `CHANGELOG.md`. Only plan/PRD text references the Phase 1 non-goal. `src/host_context.rs` `ShellFamily` is runtime detection, not user-facing snippets.
  - **Completed**: 2026-04-26

### Verification

- [x] **1.13 Phase 1 verification: integration and quality gates** `[P0]` `[M]`
  - **Depends on**: All prior tasks in Phase 1 (1.1–1.12)
  - **Requirements**: SC-4, QG-1, QG-2, QG-3, QG-4, QG-5
  - **Acceptance Criteria**:
    - [x] All Phase 1 tasks 1.1–1.12 marked complete or intentionally skipped with reason.
    - [x] `cargo test --no-default-features --locked` passes (QG-1) — **verified 2026-04-26**
    - [x] `cargo clippy --no-default-features --locked --all-targets -- -D warnings` passes (QG-2) — **verified 2026-04-26** (integration tests are targets; use `--all-targets` to match CI-style coverage)
    - [x] `cargo build --locked` passes (QG-3) — **verified 2026-04-26**
    - [x] `cargo fmt --check` passes (QG-4) — **verified 2026-04-26**
    - [x] Code review completed for execution and policy paths (QG-5).
    - [x] Manual spot-check: default `ask` flow matches US-1/US-2 in a real terminal; verbose path matches US-3.
  - **Notes**: `cargo fmt --check` is a PRD quality gate even if not yet in `.github/workflows/ci.yml`; consider adding it to CI as part of this phase or a fast-follow.
  - **Completed**: 2026-04-26

## Phase 2: Flags, docs hardening, edge-case coverage (6/6 tasks complete)

> Tightens operator documentation, adds optional capture/preview controls, and expands automated tests for edge cases called out in the PRD’s Phase 2.
>
> **Goal:** Phase 2 is independently shippable documentation and robustness work atop Phase 1 without changing Phase 1’s core safety posture.

### Product and docs

- [x] **2.1 Documentation pass: execution modes, TTY vs capture, troubleshooting** `[P1]` `[M]`
  - **Depends on**: Task 1.13
  - **Requirements**: FR-6, NFR-4, SC-2
  - **Acceptance Criteria**:
    - [x] User-facing docs explain when streams are inherited vs captured, how Docker/bwrap differs, and where to use verbose mode for audits.
    - [x] Timeout / large-output limitations from PRD risks are addressed in docs.
  - **Notes**: Align with stakeholder table in PRD Section 4.
  - **Completed**: 2026-04-26. Added README *`clai ask`: when I/O is inherited vs captured* (table for direct/TTY, non-TTY, verbose, docker/bwrap), timeout (120s) and per-stream 256 KiB capture cap, troubleshooting bullets; `cargo fmt`, `test`, `clippy --all-targets`, `build` verified.

- [x] **2.2 Optional flag to force capture on direct runs** `[P1]` `[M]`
  - **Depends on**: Task 1.13
  - **Requirements**: PRD Phase 2 (example: “force capture”), FR-1
  - **Acceptance Criteria**:
    - [x] Operators can force piped capture even when direct+TTY would inherit, without disabling policy.
    - [x] Behavior is documented in `--help` and README.
  - **Notes**: Mitigates “TTY inheritance breaks capture-based limits” risk from PRD §8.
  - **Completed**: 2026-04-26. `select_stream_strategy(..., force_capture)`; `--force-capture` / `ask_force_capture` / `CLAI_ASK_FORCE_CAPTURE`; README table + clap help.

- [x] **2.3 Optional flag to suppress the one-line pre-execution preview** `[P1]` `[S]`
  - **Depends on**: Task 1.13
  - **Requirements**: PRD Phase 2, FR-1
  - **Acceptance Criteria**:
    - [x] Users can disable the single-line preview for scripting or minimal output.
    - [x] Default remains PRD-conformant when the flag is unset.
  - **Completed**: 2026-04-26. `--no-preview` / `ask_no_preview` / `CLAI_ASK_NO_PREVIEW`; suppresses TTY pre line and post non-TTY non-direct attribution; default unchanged when off.

### Automated coverage

- [x] **2.4 Tests: large output, truncation, and long-running child interactions** `[P1]` `[M]`
  - **Depends on**: Task 1.13
  - **Requirements**: PRD Phase 2, PRD Risk (timeout / limits)
  - **Acceptance Criteria**:
    - [x] Automated tests cover large stdout/stderr behavior and timeout termination for capture path (non-TTY) per existing `max_capture_bytes` / timeout semantics.
  - **Notes**: Keep within CI runtime budgets.
  - **Completed**: 2026-04-26. `tests/phase2_edge_cases.rs` (`capture_truncates_large_stdout` with output below pipe cap; `capture_times_out`); module comment on pipe deadlocks.

- [x] **2.5 Tests: binary or noisy output handling (policy + verbose-only warnings)** `[P1]` `[M]`
  - **Depends on**: Task 1.13
  - **Requirements**: PRD Phase 2, PRD Risk (binary/noisy output)
  - **Acceptance Criteria**:
    - [x] Tests or documented checks ensure policy still gates execution; verbose path may warn without polluting default human output.
  - **Notes**: Scope strictly to PRD Phase 2 wording—avoid new policy product surface without review.
  - **Completed**: 2026-04-26. `policy_still_blocks_obvious_destructive_proposal` + `capture_stdout_non_utf8_is_lossy`; verbose `stderr` one-line note when U+FFFD in captured strings; README *Binary or non-UTF-8* + policy bullet.

### Verification

- [x] **2.6 Phase 2 verification: quality gates** `[P0]` `[M]`
  - **Depends on**: Tasks 2.1–2.5
  - **Requirements**: SC-4, QG-1–QG-4
  - **Acceptance Criteria**:
    - [x] All Phase 2 tasks complete or skipped with rationale.
    - [x] `cargo test --no-default-features --locked` passes.
    - [x] `cargo clippy --no-default-features --locked -- -D warnings` passes (use `--all-targets` to lint integration tests; stricter than tasks text).
    - [x] `cargo build --locked` passes.
    - [x] `cargo fmt --check` passes.
  - **Notes**: QG-5: Phase 2 extends `cmd_ask` and stream strategy; same security posture (policy before run). Re-review on merge as usual.
  - **Completed**: 2026-04-26. All gates run locally; `CHANGELOG.md` Unreleased *Added (Phase 2)*.

## 3. Dependency graph

```text
1.1 (stream strategy)
└── 1.2 (inherited vs capture execution)
    ├── 1.3 (exit mapping)
    │   └── 1.6 (policy/abort semantics) ──► 1.5 (default presentation)
    ├── 1.9 (integration tests)
    └── 1.10 (perf guardrail)

1.4 (verbose opt-in) ──► 1.5 (default presentation)
1.5 ──► 1.7 (non-direct attribution)
1.3 + 1.1 ──► 1.8 (unit tests)

1.5, 1.6, 1.7 ──► 1.11 (migration + SC-2 docs)
1.11 ──► 1.12 (no shell snippets audit)
1.1..1.12 ──► 1.13 (Phase 1 verification)

1.13 ──► Phase 2 (2.1–2.6)
```

## 4. Risk mitigation tasks

Risks from PRD §8 are addressed as follows (no extra standalone tasks beyond Phase coverage):

| PRD risk                                                  | Mitigation tasks                          |
| --------------------------------------------------------- | ----------------------------------------- |
| Script users depend on old stdout / exit `0` on failure   | 1.11, 1.6                                 |
| TTY inheritance vs timeout / size limits                  | 1.2, 2.2, 2.4                             |
| Binary/noisy output                                       | 2.5, policy unchanged (1.x)               |
| Platform signal / exit mapping                            | 1.3, 1.11                                 |
| Scope creep into shell plugins                            | 1.12, Phase 3 deferred to Future Considerations |

## 5. Open questions impacting tasks

| PRD question | Affected tasks | Default if unresolved |
| ------------ | -------------- | --------------------- |
| *(none)*     | —              | PRD §9 records all resolved decisions |

## 6. Requirements coverage

| Requirement | Task(s)              | Status     |
| ----------- | -------------------- | ---------- |
| SC-1        | 1.3, 1.9, 1.13       | ✅ Covered |
| SC-2        | 1.2, 1.11, 1.13, 2.1 | ✅ Covered |
| SC-3        | 1.4, 1.5, 1.13       | ✅ Covered |
| SC-4        | 1.13, 2.6            | ✅ Covered |
| US-1        | 1.5, 1.7, 1.13       | ✅ Covered |
| US-2        | 1.3, 1.6, 1.9        | ✅ Covered |
| US-3        | 1.4, 1.13            | ✅ Covered |
| US-4        | 1.8, 1.9, 1.11       | ✅ Covered |
| US-5        | 1.12                 | ✅ Covered |
| FR-1        | 1.1, 1.4, 1.5, 2.3   | ✅ Covered |
| FR-2        | 1.1, 1.2, 1.11       | ✅ Covered |
| FR-3        | 1.3, 1.9             | ✅ Covered |
| FR-4        | 1.6, 1.11            | ✅ Covered |
| FR-5        | 1.5                  | ✅ Covered |
| FR-6        | 1.7, 2.1             | ✅ Covered |
| NFR-1       | 1.1, 1.8, 1.9        | ✅ Covered |
| NFR-2       | 1.10                 | ✅ Covered |
| NFR-3       | 1.1, 1.2, 1.6, 2.5   | ✅ Covered |
| NFR-4       | 1.11, 2.1            | ✅ Covered |
| QG-1        | 1.8, 1.9, 1.13, 2.6  | ✅ Covered |
| QG-2        | 1.13, 2.6            | ✅ Covered |
| QG-3        | 1.13, 2.6            | ✅ Covered |
| QG-4        | 1.13, 2.6            | ✅ Covered |
| QG-5        | 1.13, 2.6            | ✅ Covered |

## 7. Future considerations

- **Phase 3 / follow-up PRD:** First-party `zsh`/`fish`/`nu` snippets, optional clipboard helpers, `needs_shell` policy story, CI pseudo-TTY tests (PRD §6 Phase 3)—explicitly out of MVP task commitments above.
- **CI improvement:** Add `cargo fmt --check` to `.github/workflows/ci.yml` to match QG-4 automatically.
- **Full `clai ask` E2E:** If maintainers later add fixture/model-free E2E for `ask`, extend 1.9 patterns without replacing library-level executor tests.

---

**Review:** If any task feels too coarse or dependencies should change (especially 1.2 executor work splitting across Unix/Windows), say what to adjust and the list can be revised incrementally per the skill’s progress rules.
