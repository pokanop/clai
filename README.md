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

## Quick start

```bash
cargo run -- doctor
cargo run -- init
cargo run -- models list
cargo run -- models pull balanced-qwen25-coder-7b-q4
cargo run -- ask --print-only "list files in the current directory"
```

Paths (Unix, including macOS): config in `~/.config/clai/config.toml` (`$XDG_CONFIG_HOME/clai/` if set), data (models + `registry.json`) under `~/.local/share/clai/` (`$XDG_DATA_HOME/clai/` if set). Windows uses `%APPDATA%` for config and `%LOCALAPPDATA%\\clai` for data. Older macOS builds used `~/Library/Application Support/clai/`; that tree is still read when the XDG-style paths have no file yet. Overrides: `CLAI_*` env (figment).

### Inference (local)

Local `ask` passes your command-proposal **JSON Schema** into the chat template (Jinja), then validates the model output as JSON afterward. **GBNF grammar-constrained decoding** is optional: set `CLAI_JSON_SCHEMA_GRAMMAR=1` to enable it (uses schema-derived GBNF first, then the template’s grammar string). Leave it unset by default: some llama.cpp builds/models hit a hard abort (`GGML_ASSERT(!stacks.empty())` in `llama-grammar.cpp`) with the grammar sampler. With grammar off, use `CLAI_GRAMMAR_LAZY=1` only if you also enabled `CLAI_JSON_SCHEMA_GRAMMAR`. Cloud mode uses OpenAI-style `response_format: json_schema` when `cloud.structured_outputs` is true.

### Execution wrappers

`execution.mode` in config can be `direct` (default), `docker`, or `bwrap` (Unix only). For Docker, set `execution.docker_image` to an image that contains the tools you need; the workspace directory is bind-mounted read-write.

### Self-update and releases

`clai self update` uses [self_update](https://docs.rs/self-update) against GitHub Releases. Release assets should **include the Rust target triple** in the file name (for example `clai-x86_64-unknown-linux-gnu.tar.gz`), matching the triple this binary was built with. Override with `--target` or `CLAI_UPDATE_TARGET`. If the binary is not at the archive root, set `--bin-path-in-archive` or `CLAI_UPDATE_BIN_PATH_IN_ARCHIVE` (supports `{{ bin }}`, `{{ target }}`, `{{ version }}` per self_update).

See [.github/workflows/release.yml](.github/workflows/release.yml) for a sample release build.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.