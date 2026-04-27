# clai

Natural-language to shell command CLI: local **GGUF** inference via **[llama-cpp-2](https://crates.io/crates/llama-cpp-2)** (optional feature), Hugging Face model pulls, deterministic safety policy, and optional cloud fallback.

## Build

Requires **Rust stable**, **CMake**, and **Clang** (for `llama-cpp-sys-2`).

```bash
cargo build --release
```

- **CPU-only CI / fast iteration:** `cargo build --no-default-features`
- **Apple GPU:** `cargo build --release --no-default-features --features llama-metal`
- **NVIDIA (Linux/Windows):** `cargo build --release --no-default-features --features llama-cuda`
- **Vulkan:** `cargo build --release --no-default-features --features llama-vulkan`

Corporate TLS issues: add `--features native-tls` (see `Cargo.toml`).

### Performance (NFR-2)

`run_proposal` on the **direct** profile uses either piped **capture** or **inherited** stdio. The PRD requires that switching to the inherited path for a trivial no-op child does not add more than a **~500 ms** order-of-magnitude wall-clock overhead vs capture on **reference** hardware. A repeatable check lives in `tests/trivial_child_overhead.rs` (no Criterion; pure `std::time` medians). Run:

```bash
cargo test --no-default-features --locked --test trivial_child_overhead
```

Shared or heavily loaded CI hosts can be noisy; if this test fails intermittently, re-run locally on a quiet machine and treat the result as advisory unless the gap is clearly above 500 ms.

## Quick start

```bash
cargo run -- doctor
cargo run -- init
cargo run -- models list
cargo run -- models pull balanced-qwen25-coder-7b-q4
cargo run -- ask --print-only "list files in the current directory"
```

### Interactive session (bare `clai` on a TTY)

When **stdin and stdout** are both TTYs, `clai` with **no subcommand** (or `clai interactive`) runs a line-oriented session: each non-empty trimmed line is sent to the model like `clai ask` text. Empty lines are ignored. Built-ins: `help`, `exit` / `quit`, and `reload` (reloads the local GGUF from disk when using local inference and the `llama` build). **EOF** (Ctrl-D) ends the session with exit code **0**.

If either stdin or stdout is **not** a TTY, bare `clai` / `clai interactive` **does not** wait for input: it prints a short hint to use `clai ask '…'` and exits with code **2** (so CI and scripts do not hang).

**Execution tri-state** (how the session runs commands after policy allows them):

| Mode | Behavior |
| --- | --- |
| `dry-run` | Show the pre-run summary, then **ask** whether to execute (default **no**; policy can still require a sensitive step). |
| `confirm` | Show the summary, then prompt before run (default answer **no**). |
| `auto` | Show the summary and run without the extra “run it?” step; **policy** may still require a sensitive-operation confirm unless you pass `--yes`. |

**Precedence:** `--yes` (forces **auto** for the session and auto-confirms policy prompts) **>** `--interactive-mode` **>** `CLAI_INTERACTIVE__EXECUTION` / `[interactive]` in config (merged with other `CLAI_` env via figment) **>** legacy mapping from `policy.dry_run_default` when `[interactive].execution` is unset (`true` → `dry-run`, `false` → `confirm`).

Config example:

```toml
[interactive]
execution = "confirm"   # dry-run | confirm | auto
```

**Global flags** (see `clai --help`): `--interactive-mode`, `--yes`, `--cloud`, `--verbose` / `-v`, `--force-capture`, `--no-preview`. When placed **before** `ask`, they also apply to `clai ask` (e.g. `clai --yes ask "…"`). Subcommand-specific flags still work on `clai ask …`.

**TTY styling:** informational / ok / warn / error lines use distinct labels and color when stdout/stderr is a TTY. Set `NO_COLOR` to disable ANSI color.

**Ctrl+C:** documented as canceling the current request; use `exit` / EOF to leave the session.

Paths (Unix, including macOS): config in `~/.config/clai/config.toml` (`$XDG_CONFIG_HOME/clai/` if set), data (models + `registry.json`) under `~/.local/share/clai/` (`$XDG_DATA_HOME/clai/` if set). Windows uses `%APPDATA%` for config and `%LOCALAPPDATA%\\clai` for data. Older macOS builds used `~/Library/Application Support/clai/`; that tree is still read when the XDG-style paths have no file yet. Overrides: `CLAI_*` env (figment).

### Inference (local)

Local `ask` passes your command-proposal **JSON Schema** into the chat template (Jinja), then validates the model output as JSON afterward. **GBNF grammar-constrained decoding** is optional: set `CLAI_JSON_SCHEMA_GRAMMAR=1` to enable it (uses schema-derived GBNF first, then the template’s grammar string). Leave it unset by default: some llama.cpp builds/models hit a hard abort (`GGML_ASSERT(!stacks.empty())` in `llama-grammar.cpp`) with the grammar sampler. With grammar off, use `CLAI_GRAMMAR_LAZY=1` only if you also enabled `CLAI_JSON_SCHEMA_GRAMMAR`. Cloud mode uses OpenAI-style `response_format: json_schema` when `cloud.structured_outputs` is true.

### Execution wrappers

`execution.mode` in config can be `direct` (default), `docker`, or `bwrap` (Unix only). For Docker, set `execution.docker_image` to an image that contains the tools you need; the workspace directory is bind-mounted read-write.

### `clai ask`: when I/O is inherited vs captured

`clai` connects the child’s stdin, stdout, and stderr either to **your terminal** (inherited) or to **pipes** with a bounded read (captured). The default human path uses the same rules as the PRD’s “native shell” story: interactive terminal I/O in direct mode where it is safe to attach to the TTY; captured streams everywhere policy or tooling requires isolation or machine-oriented output.

| Situation | Stream connection | Notes |
| --- | --- | --- |
| `execution.mode = direct`, default human output, and **stdin, stdout, and stderr** are all TTYs | **Inherited** | Child output behaves like running the same argv in your terminal (colors, pagers, etc. where the tool and OS allow). |
| `execution.mode = direct`, but any of stdin/stdout/stderr is **not** a TTY (e.g. script, CI) | **Captured** | Piped I/O; `clai` can label non-direct runs and print captured stdout/stderr after the run. |
| **Verbose** (`--verbose` / `-v`, `ask_verbose` in config, or `CLAI_ASK_VERBOSE`) | **Captured** | Full proposal JSON and structured `status` / `stdout` / `stderr` for audits and logging—always capture, even on a TTY. |
| `execution.mode = docker` or `bwrap` | **Captured** | Isolated subprocess or container: streams are not forwarded to the host TTY in this phase. The tool prints a short **profile / cwd / argv** line (and verbose metadata when using `--verbose`) so output is attributable to the command (FR-6). |
| **`--force-capture`** (`ask_force_capture` in `config.toml`, or `CLAI_ASK_FORCE_CAPTURE=1`) in **direct** mode | **Captured** | Opt-in: use size-limited piped I/O and post-run printing even when all stdio are TTYs (policy unchanged). Useful when you need the same capture semantics as non-TTY runs. |
| **`--no-preview`** (`ask_no_preview` in config, or `CLAI_ASK_NO_PREVIEW=1`) | *unchanged stream choice* | Hides the single human-default **pre-run** line (`Run: …` or non-direct context) and the non-TTY attribution line for docker/bwrap; does not by itself switch inherit ↔ capture. |

**Operator / audit path:** use **`--verbose`** (or the config/env above) when you need the full `CommandProposal` and captured execution record, not the minimal default.

**Timeout and output size (risks: long runs, large streams):** `clai ask` uses a **120 second** wall-clock cap per run; if the child is still running, it is stopped and `clai` exits with **`124`** (same idea as GNU `timeout(1)` on many systems). On the **capture** path, each of stdout and stderr is read with a **256 KiB** cap per stream; excess is truncated (see the executor in `src/executor.rs`). On the **inherited** path, the child writes directly to your terminal: those byte caps do not apply, but the same **120 s** timeout still kills an unresponsive or hung child.

**Troubleshooting (quick):**

- **No colors or pager behavior** in default mode: you may be on the capture path (piped stdout, CI, or not all stdio TTYs). Run in a normal interactive terminal with direct mode, or compare with the [Manual verification: TTY (SC-2)](#manual-verification-tty-sc-2-phase-1) steps.
- **Need full JSON and stream capture for a report:** add **`--verbose`**; do not rely on default output for a structured audit trail.
- **Docker/bwrap output looks “boxed”** compared to direct: expected—those modes are capture-first; check the printed profile/cwd/argv and use verbose for a fuller record.
- **Policy:** blocked proposals still fail before any `exec`; see [`src/policy.rs`](src/policy.rs) and automated coverage (including integration tests) for high-risk patterns.
- **Binary or non-UTF-8 child output (capture path):** bytes are interpreted with a lossy UTF-8 decode; replacement characters (U+FFFD) can appear. Default human output does not add a separate **stderr** note. **`--verbose`** may print a one-line note to **stderr** when the captured `stdout`/`stderr` contains U+FFFD, without changing default human mode.

### clai ask exit codes

When `clai ask` actually **runs** a command, the `clai` process usually exits with the child’s status code. On **Unix**, if the child was terminated by a signal and the OS does not report an 8-bit code, the exit value follows the common `128 + signal` convention. If the run hits the executor’s **timeout** and the child is killed, the process exits with **`124`** (the same as GNU `timeout(1)` on many Linux systems). If the user **declines** the run confirmation, the process exits with **`2`** and no child is started. If **dry-run** applies and nothing is executed, the process exits with **`3`**. A policy block, model parse error, or other `clai` failure before any run still exits with a **non-zero** code (the policy message goes to `stderr` via the normal error path). With **`--print-only`**, a successful run exits **`0`**: no command is executed, so the exit value does not represent a child process. For full detail see [`src/ask_exit.rs`](src/ask_exit.rs).

**Script authors (portability):** Relying on a numeric `clai` exit to equal a **child** exit is reliable when a command was actually run in **direct** mode and the run finished without timeout (see NFR-1: automated coverage uses non-TTY runs). If you parse **stdout** from `clai ask`, the default format has changed: see [Migrating](#migrating-older-clai-ask-for-script-authors). **Windows** child processes typically expose an integer exit code; signal-style mapping above is **Unix**-oriented. Test your scripts on your target OS.

### Migrating: older `clai ask` (for script authors)

If you upgraded from a **release that always pretty-printed the proposal before running** and used **piped** capture with a `status` / `stdout` / `stderr` post-run block, the new default differs:

- **No legacy switch:** There is no flag, `CLAI_*` env, or config to restore the old default output lines or the old “always exit 0 after a successful `clai` run when the child failed” behavior.
- **Structured audit trail:** Use `--verbose` (or `ask_verbose` in config, or `CLAI_ASK_VERBOSE=1`) for full proposal JSON and the structured run report, similar to the old default visibility.
- **Exit codes:** Treat `clai`’s exit as the child’s when execution happens; do not assume `0` means the inner command succeeded. Handle **`2`**, **`3`**, and **`124`** for non-execution / timeout (see *clai ask exit codes*).
- **Breaking list:** See [CHANGELOG.md](CHANGELOG.md) for a concise checklist.

### Manual verification: TTY (SC-2, Phase 1)

CI does not run pseudo-TTY tests. On a **real interactive terminal** (not piped, not `script` output parsed as a substitute), on **macOS and Linux** separately:

1. **Direct mode:** Ensure `execution.mode` is `direct` in your config.
2. **All stdio to a TTY:** Run `clai ask` in an ordinary terminal window with `-y` (or confirm at the prompt) so a command actually runs, with **dry-run off** (or use `-y` where policy allows) so the child is not skipped.
3. **Representative command:** Use a natural-language request whose proposed command is something that **depends on a TTY** for typical behavior, for example colorized listing (`ls` with color flags) or a tool that would page when appropriate. Verify that the **observed** output behavior (e.g. color, pagination) matches your expectation for running the same argv **outside** `clai` in that terminal (stakeholder SC-2).
4. **Default output:** Confirm the child’s output is the main visible result, not a generic `stdout:` / `stderr:` report label in default (non-verbose) human mode.

If any step fails, capture `clai` version, OS, terminal app, and config (redact secrets) in issue reports.

### Self-update and releases

`clai self update` uses [self_update](https://docs.rs/self-update) against GitHub Releases. Release assets should **include the Rust target triple** in the file name (for example `clai-x86_64-unknown-linux-gnu.tar.gz`), matching the triple this binary was built with. Override with `--target` or `CLAI_UPDATE_TARGET`. If the binary is not at the archive root, set `--bin-path-in-archive` or `CLAI_UPDATE_BIN_PATH_IN_ARCHIVE` (supports `{{ bin }}`, `{{ target }}`, `{{ version }}` per self_update).

See [.github/workflows/release.yml](.github/workflows/release.yml) for a sample release build.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.