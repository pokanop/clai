# Contributing to clai

Thanks for your interest. This document describes how to build, test, and submit changes.

## Getting started

1. **Clone** the repository and install the **Rust stable** toolchain. This repo uses [`rust-toolchain.toml`](rust-toolchain.toml) with `rustfmt` and `clippy` components; `rustup` will pick that up automatically.

2. **Native dependencies (local inference):** the default build enables embedded **llama.cpp** via `llama-cpp-2`. You need:
   - **CMake**
   - **Clang** (for `llama-cpp-sys-2`)

   On Ubuntu/Debian CI, the equivalent packages are `cmake`, `clang`, and `libclang-dev`.

3. **Optional: git hooks** — run once per clone:

   ```bash
   ./scripts/install-git-hooks.sh
   ```

   The `pre-commit` hook runs `cargo fmt` and re-stages changed `.rs` files so commits include rustfmt output.

4. **Corporate TLS / proxies:** if you need the system TLS stack for Hugging Face or HTTP, build with `--features native-tls` (see [`Cargo.toml`](Cargo.toml)).

## Build variants

| Goal | Command |
| --- | --- |
| Default (CPU llama) | `cargo build --release` |
| CI / no embedded llama | `cargo build --no-default-features` |
| Apple GPU | `cargo build --release --no-default-features --features llama-metal` |
| NVIDIA | `cargo build --release --no-default-features --features llama-cuda` |
| Vulkan | `cargo build --release --no-default-features --features llama-vulkan` |

## Checks before you push

These mirror [`.github/workflows/ci.yml`](.github/workflows/ci.yml):

```bash
cargo fmt --check
cargo test --no-default-features --locked
cargo clippy --no-default-features --locked -- -D warnings
```

CI also runs a **`build-full`** job on **macOS** with `cargo build --locked` (default features, including `llama`). If you change native or llama build paths, verify that path locally or via the workflow.

Optional: performance smoke test (non-Criterion, can be noisy on shared machines):

```bash
cargo test --no-default-features --locked --test trivial_child_overhead
```

## Product specs and task lists

Larger features are often scoped under `plans/<name>/` (for example `prd.md` and `tasks.md`). See [`plans/`](plans/) for active and archived work; align substantial changes with an issue or an agreed plan when applicable.

## Pull requests

- **Target branch:** `main`.
- **Scope:** keep changes focused; match existing style, naming, and error handling in the area you touch.
- **Changelog:** add user-visible changes under `## [Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md) when the behavior of the binary or documented contract changes.

## Security

If you are reporting a **security vulnerability**, please do **not** open a public issue first. See [`SECURITY.md`](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the same terms as the project: **MIT OR Apache-2.0** (see [`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE)).
