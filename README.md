# clai

Turn natural language into **shell commands**: local **GGUF** inference ([llama-cpp-2](https://crates.io/crates/llama-cpp-2)), **Hugging Face** downloads, a **safety policy** that runs *before* any `exec`, and an optional **OpenAI-compatible** API.

`clai` produces a **JSON command proposal** from the model, validates it, applies **policy**, then may run the command—on your TTY, with **captured** output for logging/CI, or in **Docker** / **bubblewrap** (`bwrap` on Unix), depending on config.

**More detail:** [CHANGELOG.md](CHANGELOG.md) · [CONTRIBUTING.md](CONTRIBUTING.md) · [SECURITY.md](SECURITY.md) · [.env.example](.env.example)

## Build

You need **Rust stable**, **CMake**, and **Clang** (for the bundled llama.cpp bindings).

```bash
cargo build --release
```


| Use case                      | Command                                                               |
| ----------------------------- | --------------------------------------------------------------------- |
| Default (CPU, embedded llama) | `cargo build --release`                                               |
| No llama (CI / slim)          | `cargo build --no-default-features`                                   |
| Apple GPU                     | `cargo build --release --no-default-features --features llama-metal`  |
| NVIDIA                        | `cargo build --release --no-default-features --features llama-cuda`   |
| Vulkan                        | `cargo build --release --no-default-features --features llama-vulkan` |
| Corporate proxy TLS           | add `--features native-tls`                                           |

Optional: `./scripts/install-git-hooks.sh` — `pre-commit` runs `cargo fmt` on Rust files.

## Install

After a release build, the binary is at `target/release/clai` (`.exe` on Windows).

- **From a clone (typical):** install into Cargo’s binary directory (usually `~/.cargo/bin`; rustup adds this to your `PATH` by default):

  ```bash
  cargo install --path . --locked
  ```

  If you are not using the default feature set, pass the same flags as in the build table (for example `--no-default-features --features llama-metal` on Apple Silicon).

- **Manual:** copy or symlink `target/release/clai` to a directory on your `PATH`, or prepend `target/release` to `PATH` while developing in this repository.

Prebuilt archives from GitHub Releases are documented under [Self-update](#self-update); unpack and place the binary on your `PATH` if you prefer not to build from source.

## Quick start

If you have not [installed](#install) the binary, use `cargo run --` in place of `clai` (for example `cargo run -- doctor`).

```bash
clai doctor
clai init
clai models list
clai models pull <catalog-id>    # e.g. from `models list`
clai ask --print-only "list files in the current directory"
```

- **Catalog:** built-in [assets/registry.json](assets/registry.json); refresh with `clai models update-registry` or set `CLAI_REGISTRY_URL`. Add extra Hugging Face models with `[[models.extra]]` in `config.toml` (same fields as the registry). `clai models ollama` lists local Ollama tags for discovery; `clai` still uses GGUF files locally (or a path/cloud model from config) unless you wire cloud mode.

## Configuration

- **Config file:** TOML. **Unix (incl. macOS):** `~/.config/clai/config.toml` or `$XDG_CONFIG_HOME/clai/config.toml`. **Windows:** under `%APPDATA%` (e.g. `…\AppData\Roaming\clai\config.toml`). **macOS:** if that path has no file yet, `~/Library/Application Support/clai/config.toml` is still read. **Data dir:** `~/.local/share/clai` on Unix (`$XDG_DATA_HOME/clai` if set); on Windows, `%LOCALAPPDATA%\clai`. **Override path:** `clai --config <path>` uses only that file (no project layering). **Project layering:** with the default config path, `clai` also merges `clai.toml` and `.clai/config.toml` starting at the current working directory and walking up to the filesystem root; inner (closer) files override outer ones, and `CLAI_*` env vars override all files.
- **Policy confirmations:** read-only commands such as `ls` are not treated as sensitive. For commands the policy marks sensitive (e.g. `chmod`), you can list trusted basenames under `[policy].trusted_programs` in global or project config to skip the extra policy confirmation (blocked or `needs_shell` proposals never use this bypass). In interactive **confirm** mode, when both policy and the session would prompt, you get a **single** combined confirmation instead of two. After you approve on a TTY, `clai` may offer to **remember** the executable: append to `trusted_programs` and/or, in **interactive** confirm mode, to `[interactive].remember_run_programs` (skips the per-line run prompt only). Saves go to the **current directory’s** `clai.toml` / `.clai/config.toml` (whichever already exists, else a new `clai.toml`) or your **global** config. There is no remember offer for `needs_shell` proposals.
- **Env:** any `CLAI_*` key merges with the file (see `[.env.example](.env.example)`). Hugging Face token, cloud, registry URL, and self-update overrides are set there.
- **Interactive + local (GGUF):** optional eager load at session start: `[interactive]` table, key `local_warmup` = `off` (default) or `blocking`, or `CLAI_INTERACTIVE__LOCAL_WARMUP=off|blocking`. Use `off` on memory-constrained hosts or when you want a fast-to-prompt session shell; use `blocking` to pay load time before the first `clai>` line.

## Interactive session

If **stdin and stdout are both TTYs**, running `clai` with **no subcommand** (or `clai interactive`) starts a line-oriented loop: each line is like `clai ask` text. Built-ins: `help`, `exit` / `quit`, `reload` (reloads the GGUF when using embedded local inference). **Ctrl-D** ends with exit **0**.

If **either** stream is not a TTY, bare `clai` prints a hint and exits **2** (so scripts do not block).

**Execution mode** for the session (after policy allows a command): `dry-run` | `confirm` | `auto`. Set in `[interactive]` in config, `CLAI_INTERACTIVE__EXECUTION`, or `--interactive-mode`. `**--yes`** forces **auto** and auto-confirms policy prompts. If `[interactive].execution` is missing, the old `policy.dry_run_default` still maps: `true` → `dry-run`, `false` → `confirm`.

**Confirm-mode memory (interactive only):** In **`confirm`**, after you approve a line, you may be prompted to **remember** the executable in two ways: `[policy].trusted_programs` skips extra **policy** confirmation later (when the command stays allowed), and `[interactive].remember_run_programs` skips the **“Run proposed command?”** step on later lines while staying in confirm mode. You choose project vs global config when saving; neither applies to `needs_shell` proposals. **`auto`** never shows the run prompt, so run-confirm memory does not apply; **`dry-run`** does not execute in the same way.

**Global flags** (see `clai --help`): include `--cloud`, `--verbose`, `--force-capture`, `--no-preview` — when placed *before* `ask`, they apply to `clai ask` too. `**NO_COLOR`** disables ANSI styles.

### Local GGUF loading: interactive vs `clai ask`

With **embedded local inference** (Cargo feature `llama-embed`, on by default via the `llama` CPU backend or explicitly via e.g. `llama-metal`), local mode loads the GGUF from disk when needed:

- **Interactive session** ([`run_interactive_session`](src/session.rs)): the model is **not** loaded at startup. The first line that runs a **local** completion (after session begin) calls [`LocalLlamaSession::open`](src/engine/llama.rs) if no session exists yet, which performs the full load. **Later lines** reuse the same [`LocalLlamaSession`](src/engine/llama.rs) for the lifetime of that process—weights are not opened again for every line. The only other disk reload is the **`reload`** built-in, which re-reads the GGUF from the resolved path (or loads it if the session was still empty). Cloud mode does not use this path.
- **`clai ask`**: each run is a **separate process**. [`complete_local_with`](src/engine/llama.rs) opens a session, completes once, and exits, so **each invocation** may pay a full cold load. For many back-to-back one-shots, prefer an interactive session or expect per-process load cost. There is no built-in cross-process model daemon in `clai` today.

#### Measuring time-to-first-token (before/after changes)

Use one machine, one `clai` binary (same `cargo build` or release artifact), and one GGUF path. Record the **git commit** and a short **hardware** note (CPU model, RAM, `CLAI_N_GPU_LAYERS` if any) in team notes (OQ-4) so “baseline” and “after” are comparable.

1. **First line (cold or post-warmup):** in an interactive local session, use `/usr/bin/time` or a wall clock, or `date +%s%3N` in another terminal for rough ms: note the moment you press Enter on the *first* NL request until the **first** streamed character appears (if `CLAI_NO_STREAM` is unset) or until JSON/proposal text starts on stderr.
2. **Second and later lines:** repeat for the *second* line in the same session (model already resident); compare median over several lines for steady-state.
3. **One-shot `clai ask`:** time from process start to first token for comparison with interactive (expect full load each process unless you use cloud).

Example (rough wall-clock; replace with your shell’s monotonic time if you prefer):

```bash
# Terminal A: start session (blocking warmup optional: CLAI_INTERACTIVE__LOCAL_WARMUP=blocking)
clai
# Type first request, then second — note delays with a stopwatch or `date` in Terminal B
```

For finer-grained phase visibility, use **`clai -v`** (or `ask_verbose` / `CLAI_ASK_VERBOSE`) in interactive or `clai ask` so local inference logs **loading weights** → **initializing context** → **generating** on stderr (no API keys).

See also [docs/local-inference-engine.md](docs/local-inference-engine.md) and [docs/performance-baseline.md](docs/performance-baseline.md).

## `clai ask`

**Execution backend:** `execution.mode` = `direct` (default), `docker`, or `bwrap` (Unix). Docker uses `execution.docker_image` and bind-mounts the workspace.

**Stdio:** In `**direct`** mode, if stdin, stdout, and stderr are all TTYs and you are not in **verbose** mode, the child can use **inherited** stdio (normal terminal colors/pagers). Otherwise streams are **piped** and read with limits. `**--verbose`** always captures. `**--force-capture**` forces pipes in `direct` even on a TTY. Docker/bwrap use capture; a short run context line is printed so you can see cwd/command.

**Limits:** per-run **timeout** (child killed → exit **124** on timeout). On the capture path, stdout/stderr are capped per stream (see [src/executor.rs](src/executor.rs)); inherited TTY output has no byte cap, but the timeout still applies.

**Local inference:** the model is guided toward JSON matching the command schema. Optional: `CLAI_JSON_SCHEMA_GRAMMAR=1` for GBNF-constrained sampling (off by default; some llama.cpp/model combos crash with grammar enabled). If grammar is on and you need lazy grammar behavior: `CLAI_GRAMMAR_LAZY=1` (only with grammar on). **Cloud** can use `response_format: json_schema` when `cloud.structured_outputs` is true.

## Exit codes


| Situation                      | Code                         |
| ------------------------------ | ---------------------------- |
| Child exited normally          | Child’s exit status          |
| Child timed out (executor cap) | `124`                        |
| User declined confirmation     | `2`                          |
| Dry-run, nothing executed      | `3`                          |
| `--print-only` success         | `0` (no child run)           |
| Policy/model error before run  | Non-zero (message on stderr) |


On Unix, signal-terminated children may follow the usual `128 + signal` convention. See [src/ask_exit.rs](src/ask_exit.rs) for the full rules. For **scripting**, use `**--verbose`** if you need structured proposal + run logs; the default human output is not a stable machine format. **Default stdout shape and process exit** changed in recent releases: see [CHANGELOG.md](CHANGELOG.md#unreleased) and the **Unreleased** notes.

## Self-update

`clai self update` fetches from GitHub Releases via [self_update](https://docs.rs/self-update). Release archives should name assets with a **target triple** (e.g. `clai-x86_64-unknown-linux-gnu.tar.gz`). Use `--target` / `CLAI_UPDATE_TARGET` and optional `CLAI_UPDATE_BIN_PATH_IN_ARCHIVE` if the binary is not at the archive root. Release automation: [.github/workflows/release.yml](.github/workflows/release.yml).

## Development

[rust-toolchain.toml](rust-toolchain.toml) pins stable + `rustfmt` / `clippy`. CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs `fmt --check`, `test --no-default-features --locked`, and `clippy` on Linux and macOS. Details: [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.