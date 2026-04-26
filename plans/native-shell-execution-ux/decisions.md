# Implementation decisions: native shell execution UX

## 2026-04-26 — Task 1.1: Stream strategy module

- **Inheritance bar (`UserTerminalContext::all_streams_tty`)**: Direct + human + inherit only when stdin, stdout, and stderr are all TTYs. Requiring all three avoids treating half-interactive cases (e.g. piped stdin) as full terminal inheritance; stderr+stdout without stdin would be a looser rule if we revisit after manual SC-2 checks.
- **`OutputIntent`**: Named to align with PRD “human default vs verbose” (FR-1) without wiring CLI flags here (task 1.4 will connect flags/config).

## 2026-04-26 — Task 1.2: Inherited stdio in executor

- **`run_proposal(..., stream_strategy)`**: New parameter; `StreamStrategy::Inherit` sets `Stdio::inherit()` for stdin/stdout/stderr (direct mode only). Invalid `Inherit` with Docker/bwrap returns `AppError::Msg` at the start of `run_proposal`.
- **`RunOutcome` on inherit**: `stdout`/`stderr` stay empty in the struct because output went to the terminal; status and `timed_out` remain the source of truth for the process result.
- **Timeout / kill**: Same `wait_timeout` + kill (Unix) / job terminate (Windows) path as capture; no behavioral split.
- **Unix `pre_exec` / Windows job**: Still applied for all `ExecutionMode::Direct` spawns, regardless of `StreamStrategy`, as documented in `executor.rs` module comment.

## 2026-04-26 — Task 1.3: `clai ask` process exit = child (FR-3)

- **`RunOutcome::clai_ask_process_exit`**: Filled from `clai_ask_process_exit_for_child` for completed runs, or `CLAI_ASK_TIMEOUT_EXIT` (124) on executor timeout. `cmd_ask` calls `std::process::exit` after any post-exec human/verbose output (see task 1.5).
- **Signal / no 8-bit code (Unix)**: `128 + signal` when `ExitStatus::code()` is `None` and a signal is available; otherwise `1` (see `ask_exit` + README).
- **Policy/abort/dry-run/print-only**: See task 1.6 (decline/dry-run use dedicated exit codes; policy `Err` → 1; print-only `Ok` → 0 with documented meaning).

## 2026-04-26 — Task 1.4: Verbose `ask` opt-in

- **`verbose_ask`**: `cli --verbose` / `-v` **or** `ask_verbose: true` in `config.toml` **or** `CLAI_ASK_VERBOSE` (clap `env` on the flag; bool parsing per clap). Any sets machine-style pre-exec pretty proposal + `OutputIntent::Verbose` (captured streams).
- **`--print-only`**: Handled before the verbose pre-print branch: always one `Proposed:` + `(print-only; not executed)` then `Ok(())` — normal default still does not pre-print proposal when not print-only and not verbose.
- **Post-exec output**: See task 1.5 (human unlabeled or inherit silent; verbose structured block).

## 2026-04-26 — Task 1.5: Default human `ask` presentation

- **Pre-exec one line**: `Run: {program+args}` via `ask_command_line_preview` (shell-style quoting for display), only when not verbose and `stdout().is_terminal()` so pipes/CI stay quiet. Omits `reason`, `cwd`, and other model fields (FR-5).
- **Post-exec (human)**: `StreamStrategy::Inherit` — no extra `clai` print (I/O was already the terminal). `StreamStrategy::Capture` — raw `print!(stdout)` / `eprint!(stderr)` without `status:`/`stdout:` labels. **Verbose** still uses the structured block.

## 2026-04-26 — Task 1.6: No fake child success on non-run paths (FR-4, US-2)

- **Policy block**: Unchanged `Err` → `main` → exit `1` (message via `?` display).
- **User declines**: `CLAI_ASK_USER_DECLINED_EXIT` = **2** after `Aborted.`
- **Dry-run**: `CLAI_ASK_DRY_RUN_EXIT` = **3** after `(dry-run; not executed)`.
- **`--print-only`**: Still `return Ok(())` (exit 0) — documents as “print succeeded; not a child code” in README; not grouped with “dry-run” exit to avoid breaking explicit print-only use.

## 2026-04-26 — Task 1.7: Non-direct attribution (FR-6)

- **docker / bwrap**: `non_direct_context_one_line` — `profile=`, `image=` (docker default `alpine:latest`), `cwd=`, and argv. Shown pre-exec for human+TTY, or pre-streams when not a TTY (so CI still sees context). **Direct** still uses the plain `Run:` pre line only.
- **Verbose**: `non_direct_context_verbose` before `status:` / streams, with optional `docker_extra_args` / `bwrap_extra_args` lines.
- **FR-5**: No `reason` in attribution lines; only argv + config execution fields.

## 2026-04-26 — Task 1.8: Unit tests for stream selection + exit mapping (QG-1, US-4)

- **Stream strategy**: Added `all_streams_tty` / `OutputIntent::default` tests; existing matrix for `select_stream_strategy` retained.
- **Exit mapping**: `ask_exit` Unix-only `sh -c 'kill -TERM $$'` asserts `128+15` without PTY. **Executor** `sh -c "exit 7"` / `cmd` equivalent asserts `status` and `clai_ask_process_exit` both `7` on the capture path.

## 2026-04-26 — Task 1.9: `tests/` direct-path exit propagation (non-TTY)

- **`tests/direct_path_exit_propagation.rs`**: `run_proposal` + synthetic `CommandProposal` (`true` / `false` / `sh -c` / `cmd`) with `UserTerminalContext` all false in doc comments. Compares `OutputIntent::Verbose` → `select_stream_strategy` → `Capture` to explicit `Capture` for the same `exit 9` child. **Inherit** tests assert empty `stdout`/`stderr` and matching `clai_ask_process_exit`. No pty/PTY helpers.

## 2026-04-26 — Task 1.10: NFR-2 trivial-child overhead (median capture vs inherit)

- **`tests/trivial_child_overhead.rs`**: 20× median wall time for `StreamStrategy::Capture` vs `Inherit` for `true` / `cmd exit 0`; `assert!` `median_inherit - median_capture <= 500ms` (saturating). **README** section documents the command; warns shared CI can be noisy.

## 2026-04-26 — Task 1.11: Migration + SC-2 checklist (NFR-4, SC-2)

- **`CHANGELOG.md`**: Unreleased breaking bullets for `ask` (output, exit, no legacy), pointer to README.
- **README:** *Migrating* (no flag), *Manual verification: TTY* (macOS + Linux, direct, TTY, representative color/pager behavior), *Script authors (portability)* for Unix `128+signal` vs **Windows** integer codes.

## 2026-04-26 — Task 1.12: No first-party shell paste-in (US-5)

- **Repo audit:** No `zsh` / `fish` / `nushell` (or `nu`) fenced code blocks; README/CHANGELOG clean. PRD/planning docs may name shells as a deferred non-goal. Runtime `ShellFamily` in `host_context.rs` is not a documentation snippet.
- **Phase 3** remains the home for first-party install/config snippets if product adds them later.

## 2026-04-26 — Task 1.13: Phase 1 quality gates (QG-1..QG-5) + manual — **complete**

- **QG-1** `cargo test --no-default-features --locked` — pass.
- **QG-2** `cargo clippy --no-default-features --locked --all-targets -- -D warnings` — pass (include integration tests as targets).
- **QG-3** `cargo build --locked` — pass.
- **QG-4** `cargo fmt --check` — pass.
- **QG-5** — code review for execution and policy paths completed.
- **Manual** — default `ask` and verbose path spot-checked in a real terminal per README *Manual verification: TTY*; aligns with US-1, US-2, US-3.

## 2026-04-26 — Task 2.1: README execution / streams / limits (FR-6, NFR-4, SC-2)

- **Placement:** New subsection after *Execution wrappers*, before *clai ask exit codes*, so migration and exit-code sections stay in logical order.
- **Numeric limits:** Documented the same values `cmd_ask` passes today: `Duration::from_secs(120)` and `256 * 1024` bytes per stream on the capture path (`src/main.rs`), with inherit path noted as unbounded for volume but still subject to the 120s timeout.
- **“Stakeholder table” (tasks 2.1 note):** PRD §4 is the *Key Design Decisions* table; README prose aligns with that (default TTY connect, one-line preview, verbose opt-in, non-direct capture-first, no legacy switch).

## 2026-04-26 — Phase 2 (tasks 2.2–2.6) complete

- **2.2 `--force-capture`**: `select_stream_strategy` takes `force_capture: bool` after TTY context; `true` on `ExecutionMode::Direct` returns `StreamStrategy::Capture` before the usual direct+all-TTY inherit branch. Config mirror `ask_force_capture`; env `CLAI_ASK_FORCE_CAPTURE`. No policy/argv changes.
- **2.3 `--no-preview`**: Skips the human one-line pre-run block and the post-run non-TTY `non_direct_context_one_line` repeat. Config `ask_no_preview` / `CLAI_ASK_NO_PREVIEW`.
- **2.4 `tests/phase2_edge_cases.rs`**: Truncation test uses total child output below the typical ~64 KiB pipe buffer so the child can exit; documents deadlock risk for huge output before a single `read`. Timeout test mirrors executor timeout semantics in the integration crate.
- **2.5 Non-UTF-8**: `cmd_ask` verbose branch `eprintln!`s when U+FFFD appears in captured strings; default human unchanged. Policy integration test + README policy/binary bullets; no new `PolicyEngine` rules.
- **2.6 QG**: `cargo clippy` with and without `--all-targets` both green; `fmt`, `test`, `build` as in PRD.

## Future opportunities (not implemented)

- A slimmer TTY rule (e.g. stdout+stderr only) is possible if product wants inheritance when stdin is a pipe; document any change in this file when implementing.
