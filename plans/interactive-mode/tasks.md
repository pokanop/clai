<!-- PRD: plans/interactive-mode/prd.md -->
<!-- Generated: 2026-04-26 -->
<!-- Last Updated: 2026-04-26 -->

# Tasks: Interactive mode for clai

> Implementation tasks derived from the PRD for default TTY interactive sessions, warm local inference, pre-run proposal presentation, tri-state execution (dry-run / confirm / auto), and polished CLI output—without changing `clai ask` contracts for scripts.

## 1. Overview

### Project Summary

Today `clai ask` cold-starts local inference on every invocation (`complete_local` loads the GGUF each time), and the primary UX is hidden behind a subcommand. This initiative makes **bare `clai` on a TTY** the default interactive loop, reuses one local model load per process, shows a **rich pre-run explanation** (argv, intent, rationale, policy context) **before** any `executor` call, and gates execution behind **confirm** mode by default—with **dry-run** and **auto** configurable via CLI, env, and config with documented precedence. Non-TTY default invocations must not block (exit with a clear hint). Phase 2 adds optional polish (help, reload, `ask` output parity); Phase 3 items stay out of scope until a follow-on PRD.

### Scope Reference

- PRD: [`plans/interactive-mode/prd.md`](prd.md)
- **Phase 1 (MVP):** Default entry + session + tri-state + pre-run presentation + FR-16 ordering + local warm load + cloud parity (per PRD default) + styling + tests + docs.
- **Phase 2:** In-session help, optional `reload`, `clai ask` TTY presentation parity.
- **Phase 3 (future):** Bounded multi-turn context—explicitly deferred; not implemented here.

**Open questions affecting planning** (PRD §9): exact config/env names and `--yes` mapping (defaults assumed below); `dry_run_default` migration rule (PRD default: legacy `true` → interactive dry-run until new key set); non-TTY exit code (default exit **2**); cloud parity **P0** for Phase 1; Ctrl+C documented as cancel-then-exit; optional `cargo fmt --check` in CI (tracked as Phase 1 verification / adjacent PR).

### Task Statistics

| Metric | Count |
|--------|-------|
| Total Tasks | 21 |
| Completed | 0 |
| In Progress | 0 |
| Blocked | 0 |
| Not Started | 21 |

---

## Phase 1: MVP (default session + warm local + pre-run + tri-state)

> Deliver the interactive default path on TTY, single local model load per session, full pre-run presentation and FR-16 prompt order, config/env/CLI overrides, policy/executor parity with `clai ask`, and quality gates.
> **Goal:** Shippable interactive MVP; `clai ask` and non-TTY behavior remain safe for automation.

### Configuration and execution mode

- [ ] **1.1 Define interactive execution tri-state and effective-mode resolution** `[P0]` `[L]`
  - **Depends on**: None
  - **Requirements**: FR-13, FR-14, FR-15, FR-19, US-5, NFR-5 (resolution logic), SC-6
  - **Acceptance Criteria**:
    - [ ] Rust type represents **dry-run**, **confirm**, and **auto** with stable serde names for TOML.
    - [ ] `resolve_effective_interactive_mode` (or equivalent) implements precedence **CLI > env > config > built-in default** and is unit-testable without GGUF.
    - [ ] When the new config field is **absent**, legacy `policy.dry_run_default == true` maps to interactive **dry-run** (PRD §6 default). When absent and `dry_run_default == false`, use **confirm** for interactive mode unless CHANGELOG documents a different explicit rule—**auto** must not be the silent default for absent key.
    - [ ] When the new key is **present**, it is authoritative for interactive mode regardless of `dry_run_default`.
  - **Notes**: Extend [`src/config.rs`](../../src/config.rs) (e.g. nested `interactive` section); reuse `figment` `CLAI_` env merging already on `AppConfig::load`.

- [ ] **1.2 Config version / migration for new interactive fields** `[P0]` `[M]`
  - **Depends on**: Task 1.1
  - **Requirements**: FR-19, PRD §6 Migration
  - **Acceptance Criteria**:
    - [ ] If `config_version` bump is required, [`src/migrate.rs`](../../src/migrate.rs) includes an `apply_step` and `clai migrate` remains coherent.
    - [ ] Existing installs without the new key get deterministic defaults per Task 1.1.
    - [ ] No unbounded breaking change: old configs load or get a clear migration error with a one-line fix hint.
  - **Notes**: Today `CONFIG_VERSION_LATEST` is `1` in `config.rs`; bump only if deserialization or migration semantics require it.

### CLI routing and flags

- [ ] **1.3 Global CLI flags and `--yes` semantics for the default session** `[P0]` `[M]`
  - **Depends on**: Task 1.1
  - **Requirements**: FR-15, FR-18, PRD Open Q1 (defaults), US-5
  - **Acceptance Criteria**:
    - [ ] At least one global flag overrides interactive execution mode for the process; names appear in **top-level** `clai --help`.
    - [ ] Env override names are listed in `--help` (and README).
    - [ ] **`--yes`** on the default session is documented: maps to **auto** (force non-interactive run prompt) **and** preserves existing policy auto-confirm behavior where applicable—exact wording matches implementer choice but matches PRD default proposal.
  - **Notes**: Today [`src/main.rs`](../../src/main.rs) `Cli` has only `config`/`model` globals; mirror patterns from `Ask` flags where sensible.

- [ ] **1.4 Bare `clai` default route, TTY gating, and optional `interactive` subcommand** `[P0]` `[L]`
  - **Depends on**: Tasks 1.1, 1.3
  - **Requirements**: FR-1, FR-2, FR-12, US-1, US-8, SC-0, SC-7
  - **Acceptance Criteria**:
    - [ ] With **stdin and stdout both TTY**, `clai` with no subcommand and no extra args starts the **same** session runner as optional `clai interactive` (FR-2).
    - [ ] If stdin or stdout is **not** a TTY, the process **does not** block on input; prints concise usage or top-level help plus a one-line hint to use `clai ask '…'`; **exit code 2** (PRD FR-12 default recommendation).
    - [ ] `clap` layout uses a non-required subcommand pattern (or equivalent) so bare invocation is valid on TTY.
  - **Notes**: Current `Cli` uses `command: Commands` without `Option`; requires structural change. See [`src/main.rs`](../../src/main.rs) `run` match.

### Presentation and prompts

- [ ] **1.5 TTY message styling with severity levels and `NO_COLOR` support** `[P0]` `[M]`
  - **Depends on**: None
  - **Requirements**: NFR-2, NFR-3, US-7, SC-3, SC-4
  - **Acceptance Criteria**:
    - [ ] When stdout is a TTY, informational / success / warning / error paths are visually distinct (labels and/or color).
    - [ ] When `NO_COLOR` is set, **no** ANSI color codes are emitted for those messages.
    - [ ] Piped/non-TTY output avoids interactive-only framing where documented.
  - **Notes**: PRD allows a small crate (`anstream`, `owo-colors`, etc.) with license review; gate styling on `IsTerminal` like existing code.

- [ ] **1.6 Pre-run proposal presentation module** `[P0]` `[L]`
  - **Depends on**: Task 1.5 (for styled output; can stub initially)
  - **Requirements**: FR-4, FR-6, US-2, SC-5
  - **Acceptance Criteria**:
    - [ ] Given a parsed [`CommandProposal`](../../src/schema.rs) and policy outcome metadata, render: executable + args (or clear shell line when `needs_shell`), a **what it does** line, **why** from `reason` or an explicit **no rationale provided** message, optional `cwd`, optional confidence, policy blocked / requires-confirmation hints.
    - [ ] Presentation runs **before** any `executor` invocation for the turn (FR-6).
    - [ ] **Dry-run** interactive mode still runs presentation; only execution is omitted.
  - **Notes**: Keep pure formatting testable without subprocess; avoid duplicating policy decisions inside the formatter.

- [ ] **1.7 System prompt: encourage structured `reason` field** `[P0]` `[S]`
  - **Depends on**: None
  - **Requirements**: FR-4
  - **Acceptance Criteria**:
    - [ ] `build_system_prompt` (or shared builder) instructs the model to populate `reason` explaining **why** the command was chosen.
    - [ ] JSON-only / schema constraints remain unchanged.
  - **Notes**: Current prompt in [`src/main.rs`](../../src/main.rs) `build_system_prompt`; share between `ask` and session.

### Engine and session loop

- [ ] **1.8 Session-scoped local inference (single GGUF load per process)** `[P0]` `[L]`
  - **Depends on**: None
  - **Requirements**: NFR-1, US-1, SC-1
  - **Acceptance Criteria**:
    - [ ] New API holds `LlamaBackend` + `LlamaModel` (or equivalent) for the session; **`complete_local`-style** work reuses them for each user line.
    - [ ] `clai ask` continues to work via a thin path (e.g. one-shot wrapper calling the same low-level completion or existing `complete_local_best_effort`) without behavior regression.
    - [ ] `#[cfg(feature = "llama")]` gating matches the rest of the crate; `--no-default-features` builds stay green.
  - **Notes**: Refactor [`src/engine/llama.rs`](../../src/engine/llama.rs) `complete_local` which currently calls `LlamaBackend::init` and `load_from_file` on every call; add tests or counters where feasible without loading real GGUF in CI (PRD NFR-5).

- [ ] **1.9 Cloud completion path for interactive lines** `[P0]` `[M]`
  - **Depends on**: None (can integrate in 1.10)
  - **Requirements**: PRD Phase 1 cloud parity default, Goals §6, FR-4
  - **Acceptance Criteria**:
    - [ ] When cloud is selected for a session line, completion uses [`src/cloud.rs`](../../src/cloud.rs) with the same inputs/contract as `cmd_ask`.
    - [ ] Connection reuse is **best-effort** without changing API semantics; document actual behavior in PR notes if pooling is minimal.
  - **Notes**: PRD open question defaults to P0 if cost is small.

- [ ] **1.10 Interactive session runner (input loop, welcome, prompt prefix, EOF, built-ins)** `[P0]` `[L]`
  - **Depends on**: Tasks 1.4, 1.5, 1.6, 1.8, 1.9
  - **Requirements**: FR-3, FR-9, FR-10, FR-11, FR-17, US-6, NFR-4 (loop survives non-fatal errors—delegate specifics to 1.11), NFR-6 (no unbounded history in Phase 1)
  - **Acceptance Criteria**:
    - [ ] Repeated line input: each non-empty line is treated as an NL request consistent with `clai ask` text (document trim/empty-line behavior in README).
    - [ ] Session start prints short status: local vs cloud, effective interactive mode, model id/path summary.
    - [ ] Distinct input prompt or prefix (FR-17).
    - [ ] **EOF** ends session with exit **0** when no unrecoverable error.
    - [ ] **`exit` / `quit`** (or equivalent) ends without sending to the model; documented in `--help` and in-session help stub.
    - [ ] **Ctrl+C** behavior documented (cancel in-flight; double-press / `exit` to leave—per PRD default).
  - **Notes**: Prefer a dedicated module e.g. `src/session.rs` imported from `main`; keep `inquire` for confirmations consistent with `cmd_ask`.

### Policy, ordering, and execution

- [ ] **1.11 Policy integration, FR-16 ordering, and executor parity** `[P0]` `[L]`
  - **Depends on**: Tasks 1.6, 1.10, 1.1, 1.3
  - **Requirements**: FR-5, FR-7, FR-8, FR-16, US-3, US-4, SC-7, NFR-7
  - **Acceptance Criteria**:
    - [ ] **Blocked** proposals: show block explanation; **no** “run this?” offer (FR-5).
    - [ ] **Allowed** proposals: pre-run presentation **first**; then policy **sensitive** confirmation if needed (default **no**); then interactive **run it?** only in **confirm** mode; then `executor::run_proposal` when permitted (FR-16).
    - [ ] **Dry-run** mode: never calls `executor` for policy-approved proposals; skips execution prompts (FR-7).
    - [ ] **Auto** mode: skips interactive **run it?**; still runs sensitive policy confirm when required (FR-16, US-4).
    - [ ] **Confirm** mode: run prompt default answer is **no** (decline); declining keeps the session open (US-3).
    - [ ] Stream strategy, verbose/capture flags, and exit propagation align with `cmd_ask` for equivalent flags (US-4, FR-8).
  - **Notes**: Reuse [`src/policy.rs`](../../src/policy.rs), [`src/executor.rs`](../../src/executor.rs), [`src/stream_strategy.rs`](../../src/stream_strategy.rs); factor shared steps out of [`cmd_ask`](../../src/main.rs) rather than copying large blocks.

- [ ] **1.12 Non-fatal error handling in the session loop** `[P0]` `[M]`
  - **Depends on**: Task 1.10
  - **Requirements**: NFR-4
  - **Acceptance Criteria**:
    - [ ] Parse failures, model errors, and other non-fatal issues print a **clear labeled error** and return to the prompt.
    - [ ] Only documented unrecoverable resource failures terminate the process; behavior documented in README.
  - **Notes**: Align with tracing levels; avoid logging full user lines at `info!` unless already conventional.

### Observability and documentation

- [ ] **1.13 Extend `doctor` with effective interactive execution mode** `[P0]` `[S]`
  - **Depends on**: Task 1.1
  - **Requirements**: PRD Risk (migration confusion), US-5
  - **Acceptance Criteria**:
    - [ ] `clai doctor` output includes the **resolved** interactive mode (after overrides) or states defaults unambiguously.
  - **Notes**: [`cmd_doctor`](../../src/main.rs) already prints config snippets; add one line or section.

- [ ] **1.14 README and CHANGELOG: precedence, migration, flags, and operator hints** `[P0]` `[M]`
  - **Depends on**: Tasks 1.1, 1.3, 1.4, 1.11
  - **Requirements**: FR-15, FR-18, SC-6, PRD §6
  - **Acceptance Criteria**:
    - [ ] README documents tri-state, env/flag names, config key, and **CLI > env > config > default** order.
    - [ ] Which `ask` flags apply to default session vs `clai ask` only is explicit (FR-18).
    - [ ] CHANGELOG entry describes config migration and default `clai` behavior change.
  - **Notes**: Optional: root `README.md` only if that is the project’s user-facing doc (user rule: do not expand markdown scope unnecessarily—limit to what the PRD requires).

### Testing

- [ ] **1.15 Unit tests for mode resolution, presentation, FR-16 ordering, built-ins, and `NO_COLOR`** `[P0]` `[L]`
  - **Depends on**: Tasks 1.1, 1.5, 1.6, 1.11 (ordering helpers)
  - **Requirements**: PRD §7 Unit tests, NFR-5, QG-1
  - **Acceptance Criteria**:
    - [ ] Tests run under `cargo test --no-default-features --locked` without loading GGUF.
    - [ ] Coverage includes: precedence tables, empty `reason` copy, blocked vs allowed presentation branches, prompt-order pure functions, `exit`/`quit` line classification, `NO_COLOR` guard.
  - **Notes**: Follow `#[cfg(test)]` / `tests/` layout per repo; see [`src/schema.rs`](../../src/schema.rs) tests as pattern.

- [ ] **1.16 Integration test: non-TTY `clai` with no args does not block** `[P0]` `[M]`
  - **Depends on**: Task 1.4
  - **Requirements**: FR-12, PRD Risk (CI hang), QG-1
  - **Acceptance Criteria**:
    - [ ] Automated test (or `tests/` harness) invokes the binary with stdin/stdout **not** a TTY and **no** subcommand; process exits quickly with **code 2** (or documented alternative if maintainers change FR-12 decision—update PRD + task).
    - [ ] No deadlock waiting for input.
  - **Notes**: Reuse patterns from [`tests/`](../../tests/) (e.g. subprocess with piped stdio).

- [ ] **1.17 Phase 1 verification: quality gates and manual TTY smoke** `[P0]` `[M]`
  - **Depends on**: All prior Phase 1 tasks
  - **Requirements**: PRD §7, QG-1–QG-5, SC-1–SC-3 (manual where needed)
  - **Acceptance Criteria**:
    - [ ] `cargo test --no-default-features --locked` passes (QG-1, matches [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)).
    - [ ] `cargo clippy --no-default-features --locked -- -D warnings` passes (QG-2).
    - [ ] `cargo build --locked` passes with default features / `llama` on a reference machine (QG-3).
    - [ ] `cargo fmt --check` passes (QG-4; add to CI in same or adjacent PR if missing per PRD Open Q7).
    - [ ] Manual checklist: macOS and Linux TTY—second prompt does not repeat full cold-load feel (SC-1 subjective); severity colors and EOF/`exit` (SC-2, SC-3).
    - [ ] Code review sign-off for engine/session boundary and any new dependency (QG-5).
  - **Notes**: CI today runs test+clippy on Ubuntu+macOS without `fmt`; align with maintainers on fmt job.

---

## Phase 2: Polish and parity

> Optional enhancements after MVP; each item should be independently shippable.
> **Goal:** Better discoverability and consistency between interactive and one-shot `ask` on TTY.

### CLI / UX polish

- [ ] **2.1 In-session `help` and keybinding / escape documentation** `[P1]` `[M]`
  - **Depends on**: Task 1.10 (session runner)
  - **Requirements**: PRD Phase 2, US-6
  - **Acceptance Criteria**:
    - [ ] Built-in `help` lists built-ins, execution modes, and how to exit.
    - [ ] Documented in top-level `--help` / README cross-links.
  - **Notes**: Phase 1 may ship a minimal help string; this task expands it.

- [ ] **2.2 Optional in-session `reload` for model file changes** `[P2]` `[L]`
  - **Depends on**: Task 1.8
  - **Requirements**: PRD Phase 2, NFR-1 exception path
  - **Acceptance Criteria**:
    - [ ] Documented command reloads GGUF/backend; errors are handled without killing the session when possible.
    - [ ] Without `reload`, NFR-1 “no repeated full load” still holds for normal lines.
  - **Notes**: Only if maintainers want file-watch complexity—otherwise defer.

### `ask` parity

- [ ] **2.3 `clai ask` TTY: optional shared pre-run presentation (verbose or flag)** `[P1]` `[L]`
  - **Depends on**: Task 1.6
  - **Requirements**: PRD Phase 2, FR-18 (clarify flag surface)
  - **Acceptance Criteria**:
    - [ ] One-shot `ask` can show the same structured pre-run block as interactive when opted in (e.g. `--verbose` and/or new flag), without breaking script/non-TTY defaults.
    - [ ] `--help` documents the behavior.
  - **Notes**: Align with existing `print_only` / `verbose` semantics in [`cmd_ask`](../../src/main.rs).

- [ ] **2.4 Phase 2 verification: quality gates** `[P1]` `[M]`
  - **Depends on**: Tasks 2.1–2.3 completed in scope
  - **Requirements**: PRD §7, QG-1–QG-5
  - **Acceptance Criteria**:
    - [ ] `cargo test --no-default-features --locked` passes
    - [ ] `cargo clippy --no-default-features --locked -- -D warnings` passes
    - [ ] `cargo build --locked` passes
    - [ ] `cargo fmt --check` passes
    - [ ] Manual spot-check of new Phase 2 UX on TTY
  - **Notes**: Skip or mark Phase 2 tasks `[-]` if deferred; keep verification aligned with what shipped.

---

## Phase 3: Future (out of scope for this task list)

> **Deferred** per PRD §5 / §6: bounded multi-turn context, full-screen TUI, daemon architectures. If scope grows, open a new PRD and regenerate tasks.

---

## Dependency Graph

```
Task 1.1 (tri-state + resolution)
├── Task 1.2 (config migration)
├── Task 1.3 (CLI flags)
│   └── Task 1.4 (bare clai routing + optional interactive)
├── Task 1.13 (doctor)
└── Task 1.11 (policy + FR-16 + executor) — also depends on 1.3, 1.6, 1.10

Task 1.5 (styling) ──► Task 1.6 (presentation)
Task 1.7 (system prompt) — parallel
Task 1.8 (warm local engine) ──┐
Task 1.9 (cloud path) ─────────┼──► Task 1.10 (session runner)
Task 1.6 ──────────────────────┘
Task 1.4 ──────────────────────┘

Task 1.10 ──► Task 1.11 ──► Task 1.12 (non-fatal errors)
Task 1.11 ──► Task 1.14 (docs)

Task 1.4 ──► Task 1.16 (non-TTY integration)
Tasks 1.1–1.12, 1.15–1.16 ──► Task 1.17 (Phase 1 verification)

Phase 2: 1.10/1.8/1.6 ──► 2.1 / 2.2 / 2.3 ──► 2.4
```

---

## Risk Mitigation Tasks

| PRD risk | Mitigation task(s) |
|----------|-------------------|
| Engine refactor regressions in `clai ask` | 1.8 (thin wrapper + tests), 1.17 |
| Non-TTY default hang or wrong exit | 1.4, 1.16, 1.17 |
| `dry_run_default` vs tri-state confusion | 1.1, 1.13, 1.14 |
| Ctrl+C ambiguity | 1.10 (document), 1.14 |

---

## Open Questions Impacting Tasks

| PRD question | Affected tasks | Default if unresolved |
|--------------|----------------|------------------------|
| Exact config key / env names / `--yes` on default session | 1.1, 1.3, 1.14 | PRD §9 Q1 defaults: single env + global flag; `--yes` → auto + existing policy yes |
| `dry_run_default: true` migration vs greenfield **confirm** | 1.1, 1.2, 1.14 | Legacy configs without new key: **dry-run**; explicit new installs: **confirm** |
| Non-TTY exit 2 vs 0 | 1.4, 1.16 | **Exit 2** + hint |
| Cloud interactive P0 vs P1 | 1.9 | **P0** if small; else mark 1.9 `[-]` with rationale |
| `cargo fmt` in CI | 1.17 | Land fmt check in same or adjacent PR |
| Ctrl+C semantics | 1.10, 1.14 | Cancel first; document double-press / `exit` |

---

## Requirements Coverage

| Requirement | Task(s) | Status |
|-------------|---------|--------|
| FR-1 | 1.4 | Covered |
| FR-2 | 1.4 | Covered |
| FR-3 | 1.10 | Covered |
| FR-4 | 1.6, 1.7, 1.9 | Covered |
| FR-5 | 1.11 | Covered |
| FR-6 | 1.6, 1.11 | Covered |
| FR-7 | 1.11 | Covered |
| FR-8 | 1.11 | Covered |
| FR-9 | 1.10 | Covered |
| FR-10 | 1.10 | Covered |
| FR-11 | 1.10 | Covered |
| FR-12 | 1.4, 1.16 | Covered |
| FR-13 | 1.1, 1.2 | Covered |
| FR-14 | 1.1, 1.14 | Covered |
| FR-15 | 1.1, 1.3, 1.14 | Covered |
| FR-16 | 1.11, 1.15 | Covered |
| FR-17 | 1.10 | Covered |
| FR-18 | 1.3, 1.14 (Phase 1 docs); 2.3 (optional `ask` parity) | Covered |
| FR-19 | 1.1, 1.2 | Covered |
| NFR-1 | 1.8, 2.2 | Covered |
| NFR-2 | 1.5 | Covered |
| NFR-3 | 1.5 | Covered |
| NFR-4 | 1.10, 1.12 | Covered |
| NFR-5 | 1.15 | Covered |
| NFR-6 | 1.10 (no history Phase 1) | Covered |
| NFR-7 | 1.11 | Covered |
| US-1 | 1.4, 1.8, 1.10 | Covered |
| US-2 | 1.6, 1.11 | Covered |
| US-3 | 1.11 | Covered |
| US-4 | 1.11 | Covered |
| US-5 | 1.1, 1.3, 1.13, 1.14 | Covered |
| US-6 | 1.10, 2.1 | Covered |
| US-7 | 1.5 | Covered |
| US-8 | 1.4, 1.11 | Covered |
| SC-0 | 1.4 | Covered |
| SC-1 | 1.8, 1.17 | Covered |
| SC-2 | 1.10, 1.17 | Covered |
| SC-3 | 1.5, 1.17 | Covered |
| SC-4 | 1.5, 1.15 | Covered |
| SC-5 | 1.6, 1.11 | Covered |
| SC-6 | 1.1, 1.3, 1.14 | Covered |
| SC-7 | 1.11 | Covered |
| QG-1 | 1.15, 1.16, 1.17, 2.4 | Covered |
| QG-2 | 1.17, 2.4 | Covered |
| QG-3 | 1.17, 2.4 | Covered |
| QG-4 | 1.17, 2.4 | Covered |
| QG-5 | 1.17, 2.4 | Covered |

---

## Future Considerations

- Full-screen TUI, multi-session daemon, or cross-terminal model sharing (PRD non-goals / alternatives rejected).
- **Bounded conversation context** for follow-up prompts (PRD Phase 3; NFR-6 long-term).
- Scripted `expect`-style E2E for interactive flows—only if maintainers accept flake risk (PRD §7).
- Windows interactive UX parity beyond “no regressions” (PRD Constraints).
