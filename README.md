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

## Quick start

```bash
cargo run -- doctor
cargo run -- init
cargo run -- models list
cargo run -- models pull <catalog-id>    # e.g. from `models list`
cargo run -- ask --print-only "list files in the current directory"
```

- **Catalog:** built-in [assets/registry.json](assets/registry.json); refresh with `clai models update-registry` or set `CLAI_REGISTRY_URL`. Add extra Hugging Face models with `[[models.extra]]` in `config.toml` (same fields as the registry). `clai models ollama` lists local Ollama tags for discovery; `clai` still uses GGUF files locally (or a path/cloud model from config) unless you wire cloud mode.

## Configuration

- **Config file:** TOML. **Unix (incl. macOS):** `~/.config/clai/config.toml` or `$XDG_CONFIG_HOME/clai/config.toml`. **Windows:** under `%APPDATA%` (e.g. `…\AppData\Roaming\clai\config.toml`). **macOS:** if that path has no file yet, `~/Library/Application Support/clai/config.toml` is still read. **Data dir:** `~/.local/share/clai` on Unix (`$XDG_DATA_HOME/clai` if set); on Windows, `%LOCALAPPDATA%\clai`. **Override path:** `clai --config <path>`. For exact values on your machine, run `clai doctor`.
- **Env:** any `CLAI_*` key merges with the file (see `[.env.example](.env.example)`). Hugging Face token, cloud, registry URL, and self-update overrides are set there.

## Interactive session

If **stdin and stdout are both TTYs**, running `clai` with **no subcommand** (or `clai interactive`) starts a line-oriented loop: each line is like `clai ask` text. Built-ins: `help`, `exit` / `quit`, `reload` (reloads the GGUF when using local `llama`). **Ctrl-D** ends with exit **0**.

If **either** stream is not a TTY, bare `clai` prints a hint and exits **2** (so scripts do not block).

**Execution mode** for the session (after policy allows a command): `dry-run` | `confirm` | `auto`. Set in `[interactive]` in config, `CLAI_INTERACTIVE__EXECUTION`, or `--interactive-mode`. `**--yes`** forces **auto** and auto-confirms policy prompts. If `[interactive].execution` is missing, the old `policy.dry_run_default` still maps: `true` → `dry-run`, `false` → `confirm`.

**Global flags** (see `clai --help`): include `--cloud`, `--verbose`, `--force-capture`, `--no-preview` — when placed *before* `ask`, they apply to `clai ask` too. `**NO_COLOR`** disables ANSI styles.

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