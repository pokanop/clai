# Local inference engine (llama.cpp path)

This document supports **Task 2.1** of the local inference warmup/latency plan: where work happens per completion, and what is safe to reuse.

## Model vs context (`src/engine/llama.rs`)

| Object | Lifetime today | Notes |
|--------|----------------|--------|
| `LlamaBackend` | `LocalLlamaSession` | Initialized in `LocalLlamaSession::open`; reused for the process. |
| `LlamaModel` (GGUF) | `LocalLlamaSession` | Loaded once per open; `reload` replaces the in-memory model from disk. |
| `LlamaContextParams` | `LocalLlamaSession` (since optimization) | Built with `default_context_params()` at open/reload; **reused** for each `complete` to avoid recomputing thread counts and `n_ctx` every turn. |
| `LlamaContext` | **Per `complete` call** | Created with `LlamaModel::new_context` inside `complete_with_loaded_model`, then dropped when the call returns. |

`LlamaContext` in `llama-cpp-2` borrows the `LlamaModel` (`LlamaContext<'a>`). Storing a context next to the model in the same struct is a **self-referential** layout; the runtime-safe options are an extra crate (e.g. `ouroboros`) or upstream API changes. Until then, **we do not keep a long-lived `LlamaContext` across turns**.

## KV cache and reuse

The bindings expose `LlamaContext::clear_kv_cache` / `clear_kv_cache_seq` ([`context/kv_cache.rs` in llama-cpp-2](https://crates.io/crates/llama-cpp-2)). A future optimization could hold one context per session, clear KV between user lines, and re-decode the new prompt—subject to correctness with chat templates and grammar. That requires the self-referential ownership above or a redesign of the session API.

## Task 2.2 outcome

- **Implemented:** Session-scoped **`LlamaContextParams`**, refreshed on `reload`, so each turn does not re-query `available_parallelism()` for thread counts.
- **Not implemented:** Reusing a single `LlamaContext` across completions (reason above).
