# Latency and memory validation (local interactive)

This note satisfies **Task 2.3** for comparing “after” behavior to a documented baseline without checking in machine-specific numbers.

## Procedure

Follow [README: Measuring time-to-first-token](../README.md#measuring-time-to-first-token-beforeafter-changes). Record **commit hash**, **hardware**, **model file path**, and **build** (e.g. `cargo build --release` with which features) alongside any timings.

## Steady-state (second and later lines)

With **Task 2.2**, per-turn work still includes allocating a new **`LlamaContext`** and a full decode of the prompt; wall time is still dominated by **token generation** and **llama.cpp** for typical prompts. Median “line submit → first token” should be **no worse** than the prior build on the same setup (NFR-1), unless a change intentionally trades latency for other goals.

A formal **X% faster** claim requires A/B numbers from the same GPU/CPU, model, and build; those belong in team notes (OQ-4), not necessarily in the repo.

## Memory (SC-4, NFR-3)

Per-turn **new** `LlamaContext` allocation produces a similar peak **order of magnitude** to the pre-change path (one context per completion). A **long-lived** context that stays allocated would raise RSS; we have **not** added that. Peak RSS during interactive use should stay within **~125% of baseline** for the same model and session length when comparing builds that only add cached `LlamaContextParams` and optional blocking warmup (one extra load at start, not per line).

A separate **“higher memory / faster”** mode that keeps a `LlamaContext` alive is **not** enabled by default and is left for a follow-up that solves the self-referential ownership problem (see [local-inference-engine.md](local-inference-engine.md)).

## Defaults

`[interactive].local_warmup` defaults to **`off`** so session startup remains instant on small machines; eager load is opt-in (FR-3, OQ-1).
