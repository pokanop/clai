# PRD: Interactive mode for clai

## 1. Executive Summary

- **Problem Statement:** `clai ask` is invoked as a one-shot CLI command. Each run performs full local inference setup—including `LlamaBackend` initialization and GGUF model load in `complete_local`—which is slow and expensive for iterative use. Users who want to try several natural-language requests in a row pay that startup cost on every line, and the current output is utilitarian rather than product-grade. Most users expect **`clai` alone** to drop them into the primary experience without memorizing a subcommand.
- **Proposed Solution:** Make the **interactive session** the **default** when the user runs `clai` with **no arguments** (subject to TTY rules in FR-12). The session keeps one process alive, reuses a warm inference stack for local runs, and—**before any child process runs**—presents a **clear, human-readable breakdown** of the proposed command: what it will run, what it is for, and **why** it was chosen (using structured fields from the `CommandProposal`, e.g. `reason`, argv, optional confidence, plus policy/safety context when relevant). The tool then **asks whether to run** that command, except when the user has configured **automatic execution** or **dry-run** (see FR-13–FR-16). Complement with **structured, professional** terminal output (severity, color when appropriate). Keep **`clai ask <text>`** for scripts and automation.
- **Success Criteria**
  - **SC-0:** With stdin and stdout both connected to a **TTY**, running **`clai` with no subcommand and no additional arguments** must start the **interactive session** (same session behavior as the dedicated interactive entry point, if one exists). If stdin or stdout is **not** a TTY, behavior must follow **FR-12** (no silent “hang” on a non-interactive default invocation).
  - **SC-1:** In a local interactive session, the GGUF model (and associated backend + model object needed for generation) is initialized **at most once** per process; subsequent requests in that session do not repeat full model load from disk. *(Verifiable by code review, architecture sign-off, or a test/debug assertion on load count.)*
  - **SC-2:** The user can end the session predictably: **EOF (e.g. Ctrl-D on Unix)** and at least one explicit built-in to exit (e.g. `exit` or `quit`) with documented behavior; a clean exit returns process exit code **0**.
  - **SC-3:** When standard output is a **TTY**, the interface presents **distinct visual treatment** for at least: normal informational messages, success outcomes, user-facing warnings, and errors (e.g. via color and/or clear labels), without obscuring the primary command result stream.
  - **SC-4:** The implementation respects the **NO_COLOR** convention when the environment variable is set (no ANSI color in that case). *(https://no-color.org/)*
  - **SC-5:** For every natural-language turn that yields a **parsable** `CommandProposal` and is **not** blocked before presentation, the user is shown a **prose-level summary** **before** any `executor` call that includes: the **concrete argv** (or human-readable equivalent), a short **description of intent** (“what it does”), and the model’s **rationale** when available. **Dry-run** mode still must show that summary; it only **omits** execution afterward. If policy **blocks** the command, the user sees a **block explanation** instead of a run offer, per **FR-5**.
  - **SC-6:** The user can switch or configure the interactive session’s execution behavior from **dry-run (never run)** to **confirm-then-run** to **automatic execution** using **at most** one **config** field plus **optional** env and CLI override, with defaults and precedence **documented in `--help` and README**.
  - **SC-7:** Policy, and child execution when it occurs, follow the same **rules** as `clai ask` for the same `CommandProposal` and configuration, aside from the **intentional** interactive additions in this PRD (default entrypoint, pre-run summary, and tri-state execution mode).

## 2. Goals and Non-Goals

### Goals

1. Provide a long-lived **interactive command loop** as the **default** primary experience (`clai` with no args on a TTY), so users can issue many natural-language → command operations without paying repeated local model **cold load** per line.
2. **Never surprise-execute** in the default **confirm** mode: after each proposal, show **what** will run, **what it is for**, and **why** it was chosen, then **prompt** the user before starting the child process (except in **auto** mode or when **dry-run** forbids execution).
3. Reuse the existing **NL → JSON command proposal → policy → execute** pipeline; **insert** the rich pre-run presentation and the **run prompt** in a single, consistent order (see Section 4).
4. Make **dry-run → confirm → auto** **easy to configure** from **config**, with **clear** optional **env** and **CLI** overrides so operators and power users can switch behavior without editing code.
5. Deliver a **polished, professional** CLI experience: consistent framing, clear phases (proposal details → user decision → optional run → result), and TTY-aware styling.
6. Support **local inference** in interactive mode as a **P0**; support **cloud OpenAI-compatible** completion when `cloud` is selected and enabled, as a **P0** or **P1** per open question (default: P0 parity with `clai ask`).
7. Remain **backward compatible**: **`clai ask …`** and other existing subcommands behave as today; **non-interactive** and **script** use cases do not require the default no-arg path.

### Non-Goals

- We will **not** turn `clai` into a general-purpose multi-turn **chat** product; each turn still produces a **command proposal** (structured JSON), not free-form conversation as the primary output.
- We will **not** require a full-screen **TUI** (e.g. alternate screen, complex widgets) for the initial release; a text prompt loop with high-quality line-oriented UX is sufficient.
- We will **not** remove or deprecate **single-shot** `clai ask` in this initiative; it remains the explicit path for **one-shot** and **scripted** use.
- We will **not** change **policy semantics** (allowlists, dry-run, confirmation rules) except where needed for bug parity with `ask`.
- We will **not** guarantee **pseudo-TTY** or interactive-child testing in **CI** for this feature; non-TTY and unit/integration patterns consistent with the repo are sufficient unless a later PRD extends CI.

### Constraints

- **Language & stack:** Rust, `clap`, existing `engine`, `executor`, `policy`, `config`, and cloud client paths must be reused; avoid parallel divergent “second implementation” of `ask` logic.
- **Build & CI:** Changes must pass existing **CI jobs** (Rust stable, Linux + macOS for tests) and the **quality gates** in Section 7.
- **Licensing:** Any new terminal styling or line-editing dependency must be **MIT/Apache-2.0 compatible** (or stricter) and acceptable to the project’s dependency policy.
- **Cross-platform:** Interactive mode must be **usable** on at least **macOS and Linux**; Windows behavior should not regress other commands (full Windows UX parity may be P1 if resource-constrained).

### Scope Check

This PRD is **one cohesive initiative** (default entry + session + pre-run explanation + confirm + configurable execution + output quality). It does not exceed the split threshold.

## 3. User Stories and Requirements

### User Personas

- **P1 – Power CLI user:** Develops and debugs shell workflows; runs `clai` many times per hour; wants fast iteration and clear feedback.
- **P2 – Maintainer/operator:** Cares about consistency between interactive and automated use; needs predictable exit codes and policy behavior.

### User Stories

**US-1**  
As a **power CLI user**, I want to **start an interactive clai session by typing `clai` alone** (on a TTY) so that **I only wait for the model to load once per session** when using local inference and **I do not need a subcommand**.

- **Acceptance criteria**
  - **AC-1:** With stdin and stdout both TTYs, **`clai` with no subcommand and no extra arguments** starts the session and blocks until the user ends it or a fatal error occurs. Optional aliases (e.g. `clai interactive`) may exist but must not be the only way to start a session.
  - **AC-2:** On local inference, the implementation avoids reloading the full GGUF from disk for every prompt **after the first** in the same process (per SC-1).
- **Priority:** P0

**US-2**  
As a **power CLI user**, I want a **full explanation before anything runs**—the **command line**, what it is **for**, and **why** it was chosen—so that I can **decide** whether to run it.

- **Acceptance criteria**
  - **AC-1:** After a valid `CommandProposal` is produced, the system must **not** start the child process until after the **pre-run presentation** and any user prompts required by the active **interactive execution mode** and **policy** (see FR-12–FR-16).
  - **AC-2:** The pre-run presentation must include: the **executable and arguments** (or a clear shell line when `needs_shell` applies), a **“what it does”** line (sourced from the model’s `reason` and/or a concise summary; if `reason` is absent, the UI must state that no rationale was provided), and **optional** `cwd`, **confidence**, and **policy** status (e.g. blocked, or “requires extra confirmation” before run).
- **Priority:** P0

**US-3**  
As a **power CLI user**, I want a **Run it? (y/n)**-style step in the default **confirm** mode so that I **opt in to every execution** unless I change the mode to **auto** or **dry-run**.

- **Acceptance criteria**
  - **AC-1:** In **confirm** mode, after the pre-run presentation, the system must ask for explicit user approval before executing; the **recommended default** answer is **no** (decline run) to reduce foot-gun risk—document the actual default in `README` / `--help`.
  - **AC-2:** If the user declines, the session **stays** open for another line of input.
- **Priority:** P0

**US-4**  
As a **power CLI user**, I want **the same policy and execution rules as `clai ask`** when a command is actually run so that **I trust the interactive tool as much as the one-shot tool**.

- **Acceptance criteria**
  - **AC-1:** Proposals are built and parsed with the same `CommandProposal` path as `ask`.
  - **AC-2:** `PolicyEngine` results match `clai ask` for equivalent configuration. **Policy** may still require a **sensitive-operation** confirm even when the interactive mode is **auto**—order and stacking in **FR-16**.
  - **AC-3:** Stream strategy (inherit vs capture), verbose flags, and execution profile behavior align with `clai ask` for the same inputs.
- **Priority:** P0

**US-5**  
As a **power CLI user** or **operator**, I want to **switch between dry-run, confirm, and auto** from **config and env** (and optional CLI) so that **I can tune risk without recompiling**.

- **Acceptance criteria**
  - **AC-1:** At least one **config** key defines the **interactive execution mode** with three values: **dry-run** (never execute children for proposals that would otherwise run), **confirm** (always prompt after the pre-run presentation, except when blocked earlier), and **auto** (no interactive run prompt; still respect policy and FR-16).
  - **AC-2:** Precedence is **fully documented** (e.g. CLI > env > config default).
- **Priority:** P0

**US-6**  
As a **power CLI user**, I want to **exit the session** using **Ctrl-D (EOF)** or a **documented command** so that **I can leave quickly without hunting for hidden shortcuts**.

- **Acceptance criteria**
  - **AC-1:** EOF on stdin ends the session with exit code 0 when no in-flight unrecoverable error.
  - **AC-2:** A built-in (e.g. `exit` / `quit` / `help` behavior) is documented in `--help` and in user-facing help text in the app.
- **Priority:** P0

**US-7**  
As a **power CLI user**, I want **clear, visually distinct** progress and outcome messaging when my terminal is interactive so that **I can parse failures, policy blocks, and success at a glance**.

- **Acceptance criteria**
  - **AC-1:** TTY output meets SC-3 (levels of message severity).
  - **AC-2:** Non-TTY (piped) output avoids interactive-only clutter; behavior documented.
- **Priority:** P0

**US-8**  
As a **maintainer/operator**, I want **automation to stay stable** so that **scripts and CI using `clai ask …` or explicit subcommands are unaffected** by the new default `clai` behavior.

- **Acceptance criteria**
  - **AC-1:** `clai ask <text> …` and other non-default invocations keep their current contracts; **default no-arg** behavior applies only as specified in **FR-12** (TTY and argv rules).
- **Priority:** P0

### Functional Requirements

**FR-1:** The system must use **the interactive session** as the **default** when the process is started as **`clai` with no subcommand and no additional positional or trailing args**, **if and only if** the runtime satisfies **FR-12** (interactive default eligibility). If not eligible, the system must **not** start an input-blocking session.  
**FR-2:** The system may provide an **optional** alternate invocation (e.g. `clai interactive`) that always targets the same session implementation as **FR-1** (for users who prefer an explicit name).  
**FR-3:** The system must read **user text input** repeatedly until the session ends, treating each non-empty line as a **natural-language request** in the same sense as the `clai ask` text argument (leading/trailing whitespace and built-in line handling must be documented).  
**FR-4:** For each request, the system must produce a **command proposal** through the same logical path as `clai ask` (local or cloud, per config and flags). The **system prompt** or instructions to the model must **strongly encourage** populating the structured **`reason`** field so the **why** is available for display (wording and enforcement level are implementation details; the PRD only requires a good-faith default prompt and a clear UI when the field is empty).  
**FR-5:** The system must **apply PolicyEngine** before offering execution. If the decision is **blocked**, the system must show an **explanation** and must **not** offer “run this command” for that turn. If the decision is **allowed** but **requires confirmation** under policy, that policy confirmation is **in addition to** the interactive run prompt in **confirm** mode—**FR-16** defines ordering.  
**FR-6:** The system must present the **pre-run presentation** (command, what, why, supporting fields) **before** any `executor` call for that turn, except in **dry-run** interactive mode where no execution is offered by definition (the presentation still must occur).  
**FR-7:** The system must implement **FR-14’s** **interactive execution modes** so that: **dry-run** never invokes `executor` for a proposal that passed policy; **confirm** invokes `executor` only if the user accepts the run prompt; **auto** may invoke `executor` without a run prompt **while still** honoring **FR-16** and policy.  
**FR-8:** The system must execute **approved and permitted** proposals via the existing `executor` with **equivalent** stream and exit semantics to `clai ask` for the same mode flags, subject to this PRD.  
**FR-9:** The system must support **session exit** on stdin EOF.  
**FR-10:** The system must support at least one **explicit** session termination command (e.g. `exit` or `quit`) recognized without sending it to the model as a command proposal.  
**FR-11:** The system must print a **short welcome or status line** at session start (model source local vs cloud, execution mode **dry-run / confirm / auto**, and model id/path summary where available) appropriate for a professional CLI.  
**FR-12:** The system must only treat “**bare `clai`**” as the **interactive default** when **all** of the following are true: **(a)** the user’s invocation uses **no** clap subcommand, **(b)** the user’s invocation has **no** free-text / positional arguments beyond the program name, **(c)** **stdin** is a **TTY** and **stdout** is a **TTY** (or a documented, strict subset if maintainers add rare exceptions). If **(c)** fails, the system must print **concise** usage (or the same as `clai --help` for top-level) and **exit** with a **non-zero** code **or** a documented zero—pick one and document; **default recommendation:** **exit 2** with a one-line hint: use `clai ask '…'`. **Non-TTY** default must **not** block waiting for input.  
**FR-13:** The system must support a **single** primary **config** field for the **interactive execution** tri-state, named and documented in `config` schema (exact key left to implementers; must appear in `README` and `config` examples). The tri-state must be **mappable** from existing `policy.dry_run_default` where **true** **implies** interactive **dry-run** unless a new key overrides—see **FR-14** and Section 6 migration.  
**FR-14:** The **interactive execution mode** must support exactly these semantic levels: **dry-run** (present proposal details; never run), **confirm** (present details; ask user before run), **auto** (present details; run without a separate “run it?” step when policy allows). The **default** for new users must be **confirm** (safest) unless a maintainer decision ties legacy `dry_run_default: true` to **dry-run** for backward compatibility—**document the chosen default in migration notes**.  
**FR-15:** The system must allow **overrides** in this **precedence** order: **(1)** CLI global flags, **(2)** environment variables, **(3)** config file, **(4)** built-in default. The exact **env** and **flags** are implementation-defined but every release must list them in **top-level** `--help`.  
**FR-16:** The system must **order prompts** as follows when multiple apply: **(1)** pre-run presentation, **(2)** policy **sensitive** confirmation (if any), **(3)** interactive “run it?” in **confirm** mode only, **(4)** execution. The system must **not** run the child before **(1)** completes. In **auto** mode, **(3)** is skipped; in **dry-run** mode, **(2–4)** are skipped for execution.  
**FR-17:** The system must print a **prompt** or line prefix in interactive mode that distinguishes user input from system output.  
**FR-18:** The system must document which **flags** from `clai ask` (e.g. `--yes`, `--verbose`, `--print-only`, cloud toggle) apply to the **default session** and the **ask** subcommand, respectively, in `--help` text.  
**FR-19:** The system must implement **FR-1–FR-18** without unbounded breaking changes to `config` migration; if a new key is required, provide **automatic** defaulting from existing fields per Section 6.

### Non-Functional Requirements

**NFR-1 (Performance, local):** In interactive mode with local inference, the system must **not** call a code path that performs **full GGUF disk load and backend init** for every user line after the first successful initialization in that process, except after an explicit **documented** `reload` or error recovery path if such a feature is added.  
**NFR-2 (Usability, TTY):** When `stdout` is a TTY, the system must use **visually distinct** treatment for at least: informational, success, warning, and error **messages** (SC-3).  
**NFR-3 (Conventions):** When the environment variable `NO_COLOR` is set, the system must **not** emit ANSI color codes in user-facing message styling (SC-4).  
**NFR-4 (Robustness):** On non-fatal errors during a request (e.g. model generation failure, parse error), the system must **keep the session alive** by default and must print a **clear, labeled error**; the user can submit another line or exit. A **documented** exception is allowed for unrecoverable resource failures.  
**NFR-5 (Testability):** Core session logic (e.g. exit commands, empty-line handling, `NO_COLOR` behavior) must be unit-testable without loading a real GGUF in CI, using the same `llama` feature gating strategy as the rest of the repository.  
**NFR-6 (Resource):** Interactive mode must not **multiply** unbounded memory growth per line under normal use; if conversation history is introduced later, it must be **bounded** (see open questions).  
**NFR-7 (Usability, prompts):** The **confirm**-mode run prompt must use a **safe default** (decline) unless there is a strong product reason documented otherwise; the default must be **stable** in non-interactive or piped use (N/A when session does not start per FR-12).  

## 4. Solution Design

### Approach

**Default path:** `clai` with **no** subcommand and **no** extra args, on a **TTY** (per **FR-12**), starts the same **session runner** that optional aliases may call. The runner **shares** the `cmd_ask` core: build host context, system prompt, get model text, parse `CommandProposal`, then **policy**—but **before** `executor` it runs a **pre-run** phase: **format** the proposal (argv, `reason`, `cwd`, `confidence`, policy outcome) in a user-facing way, then branch on the **interactive execution mode** (**dry-run** / **confirm** / **auto**) and **FR-16** prompt order.

**Single-shot** `clai ask` can stay behavior-compatible for scripts; a later Phase 2 may **reuse the same pre-run presentation** in verbose/TTY for parity.

For **cloud** completion, the design should **reuse** HTTP client configuration and avoid unnecessary reconnections where the stack allows, without changing API semantics (exact pooling left to implementation).

**Output:** a **proposal presentation** module plus a small **message/styling** layer that centralize phase labels, severity, and TTY vs non-TTY. **System prompt** updates should nudge the model to fill **`reason`** (and optionally a short “effect summary”) so the **why** is usually present; the UI remains correct when the field is empty (per **FR-4** / **US-2**).

### Key Design Decisions

| Decision | Context | Options Considered | Rationale | Trade-offs |
|----------|---------|--------------------|-----------|------------|
| **Bare `clai` = interactive** vs **subcommand required** | User expectation; discoverability | (A) only `clai repl`, (B) **bare `clai` on TTY** + optional alias | **(B)** matches “just run the tool” and reduces help-doc friction. | **clap** must use `subcommand_required = false` and a default; **non-TTY** must not hang—**FR-12**. |
| **Tri-state** vs legacy **dry_run + yes** | Today `policy.dry_run_default` and `--yes` interact | (A) new key only, (B) new key with migration from `dry_run_default` | **(B)** one clear `interactive.execution` (name TBD) with migration table; `--yes` maps to “force auto for this run” or similar—**document**. | Slight config complexity. |
| **Pre-run** panel vs one-line `Run:` | Today default `ask` is minimal | (A) keep one line only, (B) **rich block** then prompt | **(B)** required by product; can reuse `print_only`-style content for `ask` later. | More terminal space per turn. |
| Refactor `complete_local` | Today each call inits backend + loads model | (A) session struct holding model+backend, (B) long-lived child process, (C) server daemon | (A) fits CLI binary, minimal ops burden; (B)(C) add IPC/complexity. | Refactor risk in `engine`—mitigate with tests and small API surface. |
| Styling dependency | No color crate in `Cargo.toml` today | (A) manual ANSI, (B) `anstream`/`yansi`/`owo-colors` | Pick a crate that respects **NO_COLOR** and is lightweight; **justify in PR** with MSRV/dependency review. | New dependency to maintain. |
| Cloud session parity | API is stateless HTTP | (A) new session each line, (B) shared client, connection reuse | (B) where possible for latency; behavior unchanged. | **Open:** exact pooling—see Section 9. |

### Architecture Overview

- **CLI layer:** top-level `clap` with **default** to session when **FR-12**; **subcommands** `ask`, `init`, `doctor`, `models`, etc. unchanged; optional **`interactive`** as alias; global flags for **execution mode override** per **FR-15**.
- **Session runner:** input loop → `complete` → **present proposal** (new module) → **policy** → **mode + FR-16** → optional `executor`; built-ins (`exit`/`quit`/`help`).
- **Engine layer:** new **session-scoped** local completion API: initialize once, then `complete` for each line; internal detail **not** fixed in the PRD beyond NFR-1.
- **Shared pipeline:** `CommandProposal` parsing (fields include **`reason`**, **confidence**—see `src/schema.rs`), `PolicyEngine`, `executor::run_proposal`, `stream_strategy`—unchanged in contract; **presentation** is additive.
- **Styling + presentation modules:** TTY detection + severity + structured proposal layout + `NO_COLOR` guard.

**New dependencies:** At most one small styling/IO helper if manual ANSI is rejected; every addition must be listed in a PR with justification per quality standards.

### Modular Design Principles

- Keep **parsing, policy, execution** free of line-editing concerns.
- Prefer **composing** existing `cmd_ask` building blocks over copying large blocks of `main.rs`.
- Expose test hooks only where needed (`#[cfg(test)]` or small pure functions).

### Security Considerations

- **Input validation:** User lines are **untrusted text**; they must be handled like `ask` input—no `eval` of shell, no injection into argv beyond the existing `CommandProposal` path.
- **Policy:** Unchanged: blocked commands must not execute; confirmations remain for sensitive operations.
- **Secrets:** Cloud API keys and env handling mirror `clai ask`; no new secret storage.
- **Audit / logging:** `tracing` may log session start/end; avoid logging full user lines at `info!` in production settings unless already conventional for `ask`.

**Session fixation:** N/A; no server-side session. Local process only.

## 5. Alternatives Considered

| Alternative | Pros | Cons | Verdict |
|------------|------|------|--------|
| **Shell wrapper** that loops and calls `clai ask` | No engine refactor | **Does not** fix repeated model load; fails SC-1. | Rejected. |
| **Default `clai` = immediate run** (no pre-run) | Fastest | Violates “explain before run” and **US-2**; unsafe. | Rejected. |
| **Long-lived sidecar** daemon (socket/IPC) | Theoretically shared across terminals | High operational complexity; out of scope. | Rejected for this PRD. |
| **TUI (full-screen)** | Rich UX | Slower to ship; not required for professional feel. | Deferred (non-goal). |

## 6. Implementation Plan

### Phased Rollout

- **Phase 1 (MVP):** **Bare `clai` → session** per **FR-12**; **tri-state** execution mode in config + env + at least one CLI override; **pre-run** presentation (what / why) **before** any `executor` call; **confirm**-mode “run it?” with safe default; **local** **single** model load per session; built-ins and EOF; TTY messaging with severity; `NO_COLOR`; **policy** + **FR-16** ordering; `clai ask` unchanged; **cloud** path if P0 in Section 9.
- **Phase 2:** Optional **in-session** `help` and **keybinding** documentation; optional `reload` for model file changes; **converge** one-shot `clai ask` TTY output with the same **pre-run** presentation in verbose (or a flag) for product consistency.
- **Phase 3 (future / optional):** **Multi-line** input or **bounded** conversation context for follow-up requests; may warrant a **separate** PRD if it grows scope or memory bounds.

### Tech Stack Alignment

- **Rust (stable)**, `clap`, `serde`, `tracing`, existing `inquire` for confirmations, `indicatif` if progress is needed for long loads—reuse before adding.
- **Package/CI:** `cargo` with `--locked` as in `Cargo.lock`; follow `.github/workflows/ci.yml` patterns.

### Migration and Compatibility

- **Config:** Introduce a **single** interactive execution-mode field. **Map** legacy `policy.dry_run_default == true` to **dry-run** when the new field is **absent**, unless a one-time migration sets **confirm** as the new explicit default for greenfield configs—**pick one rule** and document in `CHANGELOG` (see open question 2).
- **Version bump:** If `config` gains a new key, follow existing **`config_version`** / migration rules in the repo; avoid breaking existing installs without defaults.
- **Feature flags:** `llama` off builds must still compile; interactive local mode must error clearly if local inference is unavailable, consistent with `ask`.
- **Scripts:** Piping or redirecting `clai` with no args must not hang—**FR-12**.

## 7. Testing Strategy

### Testing Levels

- **Unit tests:** Input loop parsing (empty lines, `exit`/`quit`), `NO_COLOR` behavior, **execution-mode resolution** (config + env + CLI precedence, migration from `dry_run_default`), **FR-16** prompt ordering (pure functions / test doubles), **pre-run** formatting from synthetic `CommandProposal` values, message styling helpers.
- **Integration tests:** **Non-TTY, no-argument** invocation must match **FR-12** (no blocking session, documented exit). Reuse **non-TTY** patterns from `tests/` as today; test executor/policy paths through shared code without a live model where possible.
- **End-to-end:** Manual: interactive session on a **real TTY** for SC-2, SC-3, and “second prompt does not feel like cold start” (SC-1 validation). **Optional:** scripted **expect**-style test—only if low flake and agreed by maintainers.

### Validation Approach

- All new tests use the **same** `#[cfg(test)]` / `tests/` layout as the repository.
- **No** LLM in CI: mock or stub the completion layer for session loop tests.
- **Manual matrix:** at least one run each on **macOS** and **Linux** for interactive TTY before release (document in PR checklist).

### Quality Gates

**QG-1:** `cargo test --no-default-features --locked` — All unit and integration tests pass (matches current CI).  
**QG-2:** `cargo clippy --no-default-features --locked -- -D warnings` — No clippy warnings (matches current CI; consider `--all-targets` to include integration test crates, consistent with project practice).  
**QG-3:** `cargo build --locked` — Full default-feature build (including `llama`) succeeds on a reference `macos-latest` or maintainer dev machine, matching the **build-full** job intent.  
**QG-4:** `cargo fmt --check` — Formatting matches `rustfmt` (components declared in `rust-toolchain.toml`).  
**QG-5:** Code review sign-off for **session + engine** boundary and any new dependency.  

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Refactoring `engine` for session reuse introduces regressions in `clai ask` | Med | High | Keep `complete_local_best_effort` as thin wrapper; add tests; optional dual path during transition behind internal API. |
| **Users run `clai` in CI without args** and expect help; instead they get a **hang** or wrong exit | Med | Med | **FR-12**; integration test for **non-TTY** no-arg; clear message. |
| **Conflicting** legacy `dry_run_default` and new **tri-state** confuse operators | Med | Med | **Migration** table in README; `doctor` prints **effective** mode. |
| **Ctrl+C** during generation vs session exit is ambiguous | Med | Med | Document behavior: interrupt cancels current request; second interrupt or `exit` ends session. Implement consistently with Rust signal/async constraints. |
| Styling library breaks on **Windows** or **non-UTF-8** terminals | Low | Med | Gate colors on TTY + `NO_COLOR`; fall back to plain text. |
| Memory growth if context is retained across turns (future) | Low | Med | NFR-6; defer multi-turn to Phase 3 or bound history. |
| Cloud client “reuse” does not improve latency measurably | Med | Low | Keep correctness first; document if perf gain is best-effort. |

## 9. Open Questions

1. **Exact config key and env names** for the tri-state (e.g. `interactive.execution`, `CLAI_INTERACTIVE_MODE`) and whether **`--yes`** on the **default** session means “force **auto** for this process” — **Owner:** maintainers — **Impact:** **Docs and script compatibility** — **Default:** document a single env and mirror with a global flag; **`--yes`** maps to **auto** + existing policy `yes` behavior.
2. **Migration** from `policy.dry_run_default: true` (current default in code): does that imply **tri-state = dry-run** for existing configs, or do we use **confirm** for safety and only migrate from explicit user opt-in? — **Owner:** product + maintainers — **Impact:** **Breaking vs safety** — **Default proposal:** new installs get **confirm**; existing config files with `dry_run_default = true` map to **dry-run** until the user sets the new key.
3. **Non-TTY `clai` with no args:** **exit 2** vs **print help, exit 0** — **Owner:** maintainers — **Impact:** **CI/exit-code scripts** — **Default:** **exit 2** + one-line message (per **FR-12**).
4. **Cloud interactive parity (P0 vs P1)** — **Owner:** maintainers — **Impact:** **Scope for Phase 1** — **Default:** P0 if implementation cost is small (shared client + same `complete_cloud` path).
5. **Whether to add bounded multi-turn context** (previous proposal in prompt) — **Owner:** product — **Impact:** **NFR-6** — **Default:** **Out of scope** for Phase 1; Phase 3.
6. **Ctrl+C semantics** (cancel vs quit) — **Owner:** implementer + README — **Impact:** **User trust** — **Default:** cancel in-flight work first; document double-press to exit.
7. **CI: add `cargo fmt --check` to** `.github/workflows/ci.yml` **if missing** — **Owner:** maintainers — **Impact:** **QG-4** drift — **Default:** add fmt in an adjacent PR.

## 10. Appendix

### Glossary

- **REPL:** Read–eval–print loop; here, read user text → model → proposed command → policy → run → show result, repeat.
- **Cold start:** First inference setup including GGUF load and backend init in the current `complete_local` design.
- **NO_COLOR:** Unofficial standard to disable color when `NO_COLOR` is set.

### References

- Existing `clai ask` flow: `src/main.rs` (`cmd_ask`), `src/engine/llama.rs` (`complete_local`).
- `CommandProposal` fields (`reason`, `args`, `confidence`, …): `src/schema.rs` — use for the **pre-run** presentation; extend schema only if a separate PRD/phase requires new structured fields.
