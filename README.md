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

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
