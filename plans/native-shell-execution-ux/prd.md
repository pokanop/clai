# PRD: Native shell execution UX for `clai ask`

## 1. Executive Summary

**Problem statement:** Users invoke `clai ask` expecting natural language to turn into a command that runs *as if they typed it* in the current shell session. Today the experience foregrounds the tool itself: the model output is pretty-printed as JSON, and executed command output is re-labeled into `status` / `stdout` / `stderr` blocks. Child processes use captured pipes rather than the user’s terminal, so output often lacks TTY behavior (colors, pagers) and feels disconnected from the command that was run. That mismatch reduces trust and makes `clai` feel like a separate reporting app instead of a shell-native assistant.

**Proposed solution:** Rework the default `ask` execution path so the primary user-visible result is the same kind of I/O and exit status they would get from running the proposed command in their environment. Reserve structured reporting (JSON proposal, captured streams) for explicit verbose or machine-oriented modes. Where subprocess isolation remains required (non-direct execution profiles, policy, timeouts), the UX must still clearly connect output to the invoked command. Optional later work can extend toward shell-specific integrations that run suggestions inside the *interactive* parent shell, which a standalone binary cannot fully replicate by spawning alone.

**Success criteria (measurable):**

- **SC-1:** For `ask` runs that complete execution in direct mode without user abort, the `clai` process exit code must equal the executed child command’s exit code in 100% of automated test cases and manual spot checks.
- **SC-2:** When `stdout` is a TTY and execution uses the direct profile, the default output mode must connect the child’s standard streams to the user’s terminal so that interactive and TTY-gated tool behavior matches running the same argv outside `clai` in developer testing (verified by a documented manual scenario; automated coverage uses non-TTY tests in CI, per resolved decision on PTY).
- **SC-3:** In default (non-verbose) output, users must not be required to read a full structured command proposal (for example, large JSON) before seeing whether the command ran; P0 user stories in Section 3 define the exact bar.
- **SC-4:** All quality gates in Section 7 pass on every change targeting this initiative.

## 2. Goals and Non-Goals

### Goals

1. Align default `clai ask` behavior with the mental model: “suggested command runs here, output is the command’s output.”
2. Preserve and surface safety: policy evaluation, confirmation for sensitive operations, and clear messaging when execution is blocked or declined.
3. Make machine-readable and debug views opt-in so operators and power users can still inspect proposals and status without polluting the default path.
4. Keep compatibility with existing configuration (execution mode, policy, cloud/local inference) without silent behavior changes that bypass safety.

### Non-Goals

- **We will not** claim that a `clai` child process is literally the same as the user’s interactive shell session: aliases, shell functions, and session-local state in the parent shell are out of scope for this PRD’s core deliverables.
- **We will not** ship a full set of first-party shell plugins (zsh, fish, nu) as part of the MVP, and **we will not** add first-party example shell snippet blocks to the repository in Phase 1 (see Resolved decisions, Section 9).
- **We will not** remove the structured JSON-oriented contract between the model and the tool; we only change what is *shown by default* and how streams are handled at execution.
- **We will not** weaken the policy or allowlist model to improve convenience without an explicit, separately reviewed change.

### Constraints

- **Technical:** The project is a Rust CLI using `clap`, an executor module, and optional sandbox wrappers (`direct`, `docker`, `bwrap`); any solution must work across supported OS targets already exercised in CI.
- **Compatibility:** Default behavioral changes that affect exit codes or console output are breaking for scripts; a **migration note** is required. There will be **no** legacy or compatibility flag for old exit or output behavior (see Section 9).
- **Security:** Untrusted model output must continue to be constrained by policy before execution; new execution paths (for example, shell string execution) must remain policy-gated and off by default unless explicitly designed otherwise.

### Scope check

This PRD addresses one major initiative: default UX and stream handling for `ask` execution. It stays within a single subsystem (CLI + execution outcome presentation). If shell plugins or a separate “eval in parent shell” protocol are later requested, they should be a follow-up PRD.

## 3. User Stories and Requirements

### User Personas

- **P1 — Daily CLI user:** Uses `clai` from a terminal in a project directory; wants quick NL → command → visible result with minimal noise.
- **P2 — Automation/CI operator:** Wraps or scripts `clai`; needs stable exit codes, predictable stdout/stderr layout, and optional JSON for logging.
- **P3 — Maintainer/contributor:** Needs debuggability: proposal content, policy decisions, and stream capture in verbose modes.

### User Stories

**US-1 (P0)**  
As a daily CLI user, I want the executed command’s output to appear as the main result of `clai ask` so that the experience matches typing the command myself.

- **Acceptance criteria:**
  - Default mode does not require reading a full structured proposal before execution completes. The default path **may** print **one clean line** of feedback (for example, what will run) before the child’s output; that line must stay minimal and must not be a full JSON dump.
  - When direct mode and TTY apply, the user sees the child’s output as the primary content (not nested under a generic “tool report” template by default).
- **Priority:** P0

**US-2 (P0)**  
As a daily CLI user, I want `clai`’s exit code to reflect the command’s outcome so that scripts and shell chains behave correctly.

- **Acceptance criteria:**
  - If the child exits with code N and no abort/policy block occurred, `clai` exits with N (platform conventions for signals documented if applicable).
- **Priority:** P0

**US-3 (P1)**  
As an automation operator, I want a stable way to obtain structured proposal and execution details when I opt in so that I can log or audit what ran.

- **Acceptance criteria:**
  - An explicit flag or config-driven “verbose/JSON/report” path prints or emits structured information without becoming the default.
- **Priority:** P1

**US-4 (P1)**  
As a maintainer, I want tests that cover the default path and the verbose path so that regressions in exit code or output routing are caught in CI.

- **Acceptance criteria:**
  - Automated tests cover exit code propagation and key output-routing behavior for at least the direct path using **non-TTY** child invocations in CI. **PTY-based tests are out of scope for Phase 1.**
  - **SC-2** (TTY, pager, and color behavior) is verified via a **documented manual** check, not a CI pseudo-TTY suite in Phase 1.
- **Priority:** P1

**US-5 (P2)**  
As a power user, I want optional shell-integrated workflows (for example, copy-to-clipboard or shell function) in a **future** follow-up so that I can run suggestions in my real interactive shell when needed.

- **Acceptance criteria:**
  - **Phase 1 does not** ship or document first-party `zsh`/`fish`/`nu` paste-in snippets in the repository; future PRD may add them. No mandatory code in this PRD’s MVP.
- **Priority:** P2

### Functional Requirements

**FR-1:** The `ask` command must support a default output mode oriented toward human terminal use and at least one explicit opt-in mode for verbose or machine-oriented diagnostics, without removing policy checks. The default path **may** emit **at most one** clean line of pre-execution user feedback in addition to the child’s I/O, per stakeholder decision.

**FR-2:** In direct execution mode, when the default output mode and runtime conditions require terminal-connected I/O, the system must connect the child process’s standard input, standard output, and standard error to the user’s terminal in a way that preserves TTY behavior for standard CLI tools, subject to platform limits documented in the testing strategy.

**FR-3:** The system must propagate the child process’s exit code to the `clai` process for completed executions where no pre-exec abort occurred, except where documented platform constraints apply to signal-terminated processes.

**FR-4:** The system must retain user-visible confirmation and policy-denial flows; blocked or unconfirmed runs must not claim successful command execution.

**FR-5:** The system must not print secrets or policy bypass instructions; any new logging or debug output must follow existing project patterns for redaction or omission.

**FR-6:** Non-direct execution profiles (for example, container or sandbox wrappers) may continue to use captured or translated streams, but the user-facing summary must name the program, working directory, and profile so output is attributable to the invoked command.

### Non-Functional Requirements

**NFR-1 (Reliability):** Exit code propagation and stream routing for the direct path must be covered by **non-TTY** automated tests in CI. **PTY-based automated tests are explicitly out of scope for Phase 1.** Remaining profiles (Docker, bwrap) must have documented manual or integration verification before release. TTY-specific behavior (SC-2) is validated manually, not by CI pty in Phase 1.

**NFR-2 (Performance):** The default path must not add more than 500ms wall-clock overhead versus the current piped-capture path for a trivial no-op child command in local benchmarks on reference hardware (order-of-magnitude guardrail; exact harness left to implementation).

**NFR-3 (Security):** Default UX changes must not disable policy evaluation, allowlist checks, or user confirmation for sensitive commands.

**NFR-4 (Compatibility):** A short migration note must describe changes to default stdout layout and exit code behavior for users who parse `clai` output in scripts.

## 4. Solution Design

### Approach

Shift the *default* user-facing contract from “tool generates a report about a command” to “tool runs a proposed argv and the terminal shows that command’s result,” while keeping introspection and structured views available. Execution remains subprocess-based; the “native” feeling comes from **stream inheritance**, **exit code alignment**, and **reduced default framing**, not from pretending the child is the parent shell.

### Key Design Decisions

| Decision | Context | Options considered | Rationale | Trade-offs |
|----------|---------|--------------------|-----------|------------|
| Default = terminal-connected I/O in direct TTY | Users expect colors and pagers | Always capture; always inherit | Inherit in the common case so output matches a normal run | Sandboxed modes still differ; must document |
| One clean pre-exec line (optional) | Users want orientation without JSON noise | No feedback; or multi-line banner | Stakeholder chose: allow a **single** clean line before child output in default mode | Wording and exact format left to implementation |
| Opt-in verbose/structured | Power users and CI need full proposal | Verbose by default; JSON only in logs | Reduces noise for the majority | Users must learn one flag for detail |
| Exit code = child | Scripts chain `clai` with `&&` / `\|\|` | Always 0 on success of `clai` itself; map only in `--json` | Matches shell mental model | **No** legacy opt-out; migration note only (stakeholder decision) |
| Defer in-shell `eval` / plugins | True interactive shell state cannot be set by a child | Build plugins now; document only | Scope control; **no** first-party shell snippets in repo in Phase 1 | Full “parent shell” parity delayed |
| Non-direct = capture-first | Docker/bwrap cannot fully mirror host TTY | Stream-forward to host TTY in v1 | Stakeholder accepted capture-first with clearer attribution (FR-6) | “Native” feel in sandboxes lags direct mode |

### Architecture Overview

- **Input path unchanged in principle:** Natural language to model to validated `CommandProposal` structure.
- **Policy layer unchanged in intent:** Evaluate before execution; same gates for destructive patterns.
- **Execution layer:** Select between stream inheritance (default direct + TTY + human mode) and capture (verbose mode, non-TTY, or non-direct profiles), with shared timeout and size safeguards where they exist today.
- **Presentation layer:** Map outcomes to user-visible text; default minimizes tool-generated scaffolding.

**New dependencies:** None required by this PRD; use existing stdlib and project modules.

### Modular Design Principles

- Isolate “how we run the child” from “how we print the result” so future shell integrations can reuse policy and argv construction.
- Keep platform-specific TTY and signal behavior behind small, testable modules.

### Security Considerations

- **Authorization:** No change to the principle that only policy-approved programs run; any future shell-string execution (out of scope for MVP) must be separately gated, off by default, and audited.
- **Input validation:** Proposals remain structured and validated; LLM output is not treated as safe shell.
- **Data protection:** No new collection of user environment data beyond what is already passed to the model in host context unless a separate change is approved.
- **Audit trail:** Verbose/JSON mode remains the right place for detailed execution records for operators.

## 5. Alternatives Considered

| Alternative | Pros | Cons | Verdict |
|-------------|------|------|--------|
| Only improve labels (`stdout:`) without changing streams | Simple | Does not fix TTY or “detached” feel | Rejected: insufficient for SC-2 |
| Always capture and pretty-print in `clai` | Uniform across modes | Worse TTY behavior; perpetuates “report” framing | Rejected for default path |
| Primary interface = shell `eval` in parent | True shell parity | High risk, hard to policy-wrap, fragile | Rejected as default; optional docs later |
| Send proposal to editor/clipboard only | Very safe | Extra steps; not “auto execute” | Complementary, not a replacement |

## 6. Implementation Plan

### Phased Rollout

- **Phase 1 (MVP):** Default human-oriented mode with terminal-connected I/O in direct TTY where applicable; optional **one clean line** of pre-execution feedback; exit code propagation; minimal default scaffolding; opt-in verbose/structured output; **non-TTY** integration tests; migration note (no legacy flags); no repo-hosted shell paste-in examples.
- **Phase 2:** Tighten documentation and any additional flags (for example, force capture, suppress the one-line preview); expand automated coverage for edge cases (large output, binary noise policy).
- **Phase 3 (optional / future):** Optional first-party shell integration snippets, `needs_shell` policy story, or clipboard helpers; optional **CI pseudo-TTY** tests — only after Phase 1–2 are stable; may split to a separate PRD.

### Tech Stack Alignment

- Rust, `clap`, existing `executor` and `policy` modules, `cargo` for build and test, CI on Ubuntu and macOS as today.

### Migration and Compatibility

- Document changes to default console output and exit codes in README or a CHANGELOG entry.
- **No** environment variable, long option, or config flag will preserve pre-change exit code or “always capture” behavior; adopters who parse `clai` output must update scripts using the migration note only.

## 7. Testing Strategy

### Testing Levels

- **Unit tests:** argv construction, mode selection (inherit vs capture), exit code mapping helpers.
- **Integration tests:** `clai ask` with a trivial subprocess (e.g., `true` / `false`) and assertions on process exit code; **non-TTY** runs and verbose flags in CI. **No PTY in CI in Phase 1.**
- **End-to-end / manual:** TTY color and pager behavior for one representative command on macOS and Linux (SC-2); Docker/bwrap paths verified manually or in optional jobs.

### Validation Approach

- Follow existing `#[test]` and binary integration patterns in the repository.
- Add tests before or alongside behavior changes for FR-2 and FR-3.

### Quality Gates

**QG-1:** `cargo test --no-default-features --locked` — All unit and integration tests pass (matches CI).

**QG-2:** `cargo clippy --no-default-features --locked -- -D warnings` — No clippy warnings (matches CI).

**QG-3:** `cargo build --locked` — Release-style build succeeds with default features (matches `build-full` job intent).

**QG-4:** `cargo fmt --check` — Formatting matches `rustfmt` (components declared in `rust-toolchain.toml`).

**QG-5:** Code review — At least one maintainer approves, including security-sensitive execution paths.

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Script users depend on old stdout layout or clai’s exit 0 on child failure | Med | Med | Document migration; **no** compatibility flag; breaking change by policy |
| TTY inheritance breaks capture-based timeout or size limits | Med | Med | Retain non-default capture mode; document limits; tests for long-running child |
| Binary or noisy output to terminal from malicious proposal | Low | Med | Policy remains gate; consider warnings in verbose path only |
| Platform differences in signal and exit code mapping | Med | Low | Document behavior; test Linux and macOS in CI where possible |
| Scope creep into full shell plugins | Med | Med | Non-goals and phased plan; split follow-up PRD |

## 9. Open Questions

**None** — the following were resolved (stakeholder answers recorded 2026-04-26).

### Resolved decisions

| # | Topic | Decision |
|---|--------|----------|
| 1 | Default feedback line | The default path **may** print **one clean line** of feedback (for example, what will run) so users get orientation without full JSON. Exact phrasing and when to suppress (e.g. non-interactive) remain implementation details. |
| 2 | Legacy exit / output flag | **No.** No `CLAI_LEGACY_*` env, long-opt, or config to preserve pre-change exit codes or capture-only output. **Migration note only** for script authors. |
| 3 | First-party shell snippets in repo | **No** example `zsh`/`fish`/`nu` paste-in blocks in Phase 1 documentation. Optional in a **later** phase or follow-up PRD. |
| 4 | Pseudo-TTY in CI (Phase 1) | **No.** Rely on non-TTY integration tests in CI; SC-2 (true TTY behavior) by **documented manual** check. Optional CI pty may be a **later** phase. |
| 5 | Non-direct (Docker / bwrap) | **Ok** as specified: **capture-first** in Phase 1 with clearer attribution (FR-6). Host-TTY stream forwarding is **not** required in Phase 1; may be revisited later. |

## 10. Appendix (Optional)

- **Glossary:** *Direct mode* — `execution.mode` configuration that runs the program without Docker/bwrap.
