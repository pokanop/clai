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

Config: `~/.config/clai/config.toml` (or `CLAI_*` via figment). Models cache: platform data dir under `clai/models/`.

### Inference (local)

Local `ask` uses the model chat template with a **JSON Schema → GBNF grammar** from llama.cpp, so generation is constrained to the command-proposal shape (with lazy-grammar triggers when the template provides them). Cloud mode uses OpenAI-style `response_format: json_schema` when `cloud.structured_outputs` is true.

### Execution wrappers

`execution.mode` in config can be `direct` (default), `docker`, or `bwrap` (Unix only). For Docker, set `execution.docker_image` to an image that contains the tools you need; the workspace directory is bind-mounted read-write.

### Self-update and releases

`clai self update` uses [self_update](https://docs.rs/self-update) against GitHub Releases. Release assets should **include the Rust target triple** in the file name (for example `clai-x86_64-unknown-linux-gnu.tar.gz`), matching the triple this binary was built with. Override with `--target` or `CLAI_UPDATE_TARGET`. If the binary is not at the archive root, set `--bin-path-in-archive` or `CLAI_UPDATE_BIN_PATH_IN_ARCHIVE` (supports `{{ bin }}`, `{{ target }}`, `{{ version }}` per self_update).

See [.github/workflows/release.yml](.github/workflows/release.yml) for a sample release build.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
