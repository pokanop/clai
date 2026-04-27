# Decision log: local-inference-warmup-and-latency

## 2026-04-26 — Task 1.1 documentation placement

- **Context**: PRD task 1.1 asks for the cost model in README or primary user doc.
- **Decision**: Add a dedicated `###` under **Interactive session** in [README.md](../../README.md), with links to [src/session.rs](../../src/session.rs) and [src/engine/llama.rs](../../src/engine/llama.rs) for maintainers. Do not duplicate long excerpts from source; describe behavior in user terms.
- **Rationale**: README is the primary onboarding doc; subsection keeps `## clai ask` focused while placing the one-shot vs session contrast next to the interactive section.

## 2026-04-26 — Warmup default and modes (1.3 / OQ-1)

- **Decision**: `local_warmup` default **`off`**. Modes: `off` | `blocking` only in v1. No `background` enum variant until a follow-up implements threaded warmup (1.5 skipped).
- **Rationale**: PRD and OQ-1: off until benchmarked; R-2 OOM on eager load; simpler surface area.

## 2026-04-26 — Task 1.5 background warmup (skipped)

- **Decision**: Defer **background** model load and **spinner** UX. Ship **blocking** warmup plus static “Loading local model (this may take a while)…” and expanded session banner. Aligns with PRD OQ-3 default to ship blocking + static first.
- **Rationale**: Avoid cross-thread `LlamaContext` / `LlamaModel` sharing (R-1, PRD §4) without a design review; smaller merge risk.

## 2026-04-26 — QG-5 review notes (1.9 / 2.5)

- **Threading / memory**: No new threads. Warmup runs **on the main thread** before the readline loop. Each completion still creates a new **`LlamaContext`**; session holds **`LlamaContextParams`** only to cut redundant per-turn param construction (2.2). No API keys added to local verbose lines (cloud path unchanged; FR-5).
- **Manual smoke (1.9)**: Maintainer should run `clai` / `clai interactive` on a TTY with a real GGUF: `CLAI_INTERACTIVE__LOCAL_WARMUP=off` and `=blocking`, confirm banner and first-line behavior match README.

## 2026-04-26 — Long-lived `LlamaContext` (2.1 / 2.2, OQ-2)

- **Decision**: Do **not** store `LlamaContext` on `LocalLlamaSession` in this PR: the type borrows `LlamaModel` (`LlamaContext<'a>` from `llama-cpp-2`), so session-wide reuse needs a self-referential struct, extra dependency, or upstream owned handle. **Do** store **`LlamaContextParams`** in `LocalLlamaSession` and `clone` per `complete` to avoid recomputing `available_parallelism` and default `n_ctx` every line.
- **Rationale**: Documented in [docs/local-inference-engine.md](../../docs/local-inference-engine.md); `clear_kv_cache` could support reuse later if ownership is solved.

## 2026-04-26 — Task 2.3 numbers in-repo

- **Decision**: Baseline **medians/p95** and **RSS** are not committed; [docs/performance-baseline.md](../../docs/performance-baseline.md) explains procedure and that steady-state is generation-bound, with RSS expectations unchanged for one-context-per-turn.

## Future opportunities

- Background warmup + TTY “warming” indicator (Task 1.5).
- Self-referential or upstream-owned context for true **KV** reuse between lines.
- `LocalWarmupMode::Auto` (TTY + size heuristic) for OQ-1.
