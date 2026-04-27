# Changelog

## [Unreleased]

### Added (interactive mode)

- **Default TTY entrypoint:** `clai` with no subcommand (or `clai interactive`) starts an interactive line loop when stdin **and** stdout are TTYs; non-TTY invocations exit **2** with a hint to use `clai ask`.
- **Warm local load:** `LocalLlamaSession` keeps one `LlamaBackend` + `LlamaModel` per process for the session; `complete_local` remains a one-shot load for `clai ask`.
- **Tri-state execution:** `dry-run` / `confirm` / `auto` via `[interactive].execution`, `CLAI_INTERACTIVE__EXECUTION`, `--interactive-mode`, and `--yes` (forces auto + policy auto-confirm). Precedence: CLI `--yes` > `--interactive-mode` > env/config field > legacy `policy.dry_run_default` when the new key is absent.
- **Interactive `dry-run`:** Prompts whether to execute after the pre-run presentation (default **no**), instead of never running.
- **Pre-run presentation** module and **TTY** severity helpers (`NO_COLOR` respected).
- **`clai ask --verbose` on a TTY** prints the same structured pre-run block as the interactive session after policy evaluation.
- **Tests:** unit coverage for mode resolution, presentation, FR-16 helpers, built-ins, `NO_COLOR`; integration `tests/non_tty_default_invocation.rs`.
- **CI:** `cargo fmt --check` on pull requests and main.

### Added (Phase 2)

- **`clai ask --force-capture`:** In `execution.mode = direct`, force piped capture (size limits) even on a TTY, without changing policy. Config: `ask_force_capture` or `CLAI_ASK_FORCE_CAPTURE=1`.
- **`clai ask --no-preview`:** Omit the one-line pre-run `Run:` / non-direct context line (and the non-TTY docker/bwrap attribution line) in default human mode. Config: `ask_no_preview` or `CLAI_ASK_NO_PREVIEW=1`.
- **Verbose + non-UTF-8 capture:** A one-line `stderr` note when verbose output includes replacement characters from lossy decode.
- **Tests:** `tests/phase2_edge_cases.rs` (truncation, timeout, lossy output, policy block integration).

### Breaking: `clai ask` (default UX and process exit)

These changes align default `ask` with “run the proposed command; show its result and exit like the shell” (see [README](README.md#clai-ask) and the notes below). **There is no** compatibility flag, environment variable, or config key to restore the previous default **stdout shape** or **process exit** behavior.

- **Pre-execution output:** The default no longer pretty-prints the full command proposal (JSON) before every run. Use `--verbose` / `-v`, or set `ask_verbose = true` in `config.toml`, or set `CLAI_ASK_VERBOSE=1` for the prior structured pre/post `ask` view.
- **Process exit after a run:** If `clai ask` actually executes a command, the `clai` process generally exits with the **child** exit code. Pipelines and scripts that assumed `clai` would exit `0` whenever the tool itself did not error must be updated. Non-run paths use dedicated codes (e.g. `2` declined confirmation, `3` dry-run, `124` run timeout) — see [README: Exit codes](README.md#exit-codes) and `src/ask_exit.rs`.
- **Default human post-execution print layout:** After the run, default human output no longer uses a fixed `status:` / `stdout:` / `stderr:` block. Child capture output is written without those labels; TTY+direct+human uses inherited stdio. Use `--verbose` for the structured run report.
- **Docker / bwrap:** When `execution.mode` is `docker` or `bwrap`, `clai` prints a one-line run context (profile, cwd, command) so captured output is attributable.

For current command-line behavior, see [README: `clai ask`](README.md#clai-ask) and [Exit codes](README.md#exit-codes).
