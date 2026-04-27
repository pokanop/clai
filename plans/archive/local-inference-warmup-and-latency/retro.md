<!-- PRD: plans/local-inference-warmup-and-latency/prd.md -->
<!-- Tasks: plans/local-inference-warmup-and-latency/tasks.md -->
<!-- Closed: 2026-04-26 -->

# Retrospective: Local inference warmup and interactive latency (clai)

> Shipped optional blocking warmup, documentation, verbose local phases, cached `LlamaContextParams`, and baseline/validation docs—without a background-load path; one optional task was intentionally skipped.

## 1. Summary

The PRD targeted cold-start and steady-state latency in local (GGUF) interactive mode: users waited until the first line to load the model, and each completion could repeat expensive context setup. The team delivered **documentation** of load semantics and measurement, **`[interactive].local_warmup`** (`off` default, `blocking` opt-in) with main-thread warmup before the readline loop, **user-facing banner and help** for readiness, **verbose** distinction of loading / context / generation on stderr, and **engine** changes that **reuse `LlamaContextParams`** per session while still creating a new `LlamaContext` per completion. Phase 2 added **`docs/local-inference-engine.md`**, **`docs/performance-baseline.md`**, tests, and full **quality-gate** verification. **Task 1.5** (background warmup + spinner) was **skipped** in favor of blocking warmup and static copy, matching PRD OQ-3’s “ship blocking first” default. The numeric success bars in SC-1/SC-2 (50% / 20% improvements) were not recorded as in-repo measurements; the team instead shipped a **repeatable procedure** and **written analysis** where the PRD allows alternatives.

## 2. Metrics

| Metric | Value |
|--------|-------|
| Total tasks | 14 |
| Completed `[x]` | 13 (92.9% of all tasks) |
| Skipped `[-]` | 1 |
| Blocked `[!]` | 0 |
| Not started `[ ]` | 0 |
| **Effective completion rate** | **100%** (13 ÷ (14 − 1) — one task deliberately out of scope) |
| Phase 1 (tasks 1.1–1.9) | 8 ÷ (9 − 1) = **100%** of non-skipped work (1.5 optional skip) |
| Phase 2 (tasks 2.1–2.5) | **100%** (5 of 5) |
| PRD requirements (per `tasks.md` coverage table) | All rows marked **Covered** for this initiative; see **Scope drift** for nuance on quantitative SC-1/SC-2. |
| Tasks without PRD traceability | 0 (each task lists `Requirements:`) |
| Gold-plating / tasks without PRD ID | **0** intentional extras beyond PRD (see Scope drift) |

*Interpretation: **100%** effective completion reflects one **optional** P1 item (1.5) marked `[-]` by design, not a delivery failure.*

## 3. What Was Built

- README (and cross-links) explaining **when** the GGUF loads in interactive **vs** one-shot `clai ask`, and that **`LocalLlamaSession`** lasts for the process with **`reload`** as the extra disk read.
- A **repeatable** description of how to think about **time-to-first-token** and where to log **commit + hardware** (OQ-4), plus links to `docs/local-inference-engine.md` and `docs/performance-baseline.md`.
- Config/env: **`[interactive].local_warmup`** and **`CLAI_INTERACTIVE__LOCAL_WARMUP`** with values **`off`** (default) and **`blocking`**; tests for TOML parse and default **`off`**.
- **Blocking warmup** on the main thread after the session banner when `blocking` is set; failures **warn** and **fall back** to lazy load; **cloud** and **non-llama** paths unchanged in spirit (no spurious local loads when `llama` is off).
- **Session start** and **help** text describing warmup; **`.env.example`** and top-level **`clai`** doc comment updated.
- **Verbose** local path: separate stderr lines for **loading weights**, **initializing context**, and **generating** (no API keys on cloud path).
- **Engine:** `LocalLlamaSession` holds **`LlamaContextParams`** (rebuilt on **`reload`**) to avoid recomputing thread/`n_ctx` every line; one **`new_context` per `complete`** remains.
- **Tests:** config tests; **`default_context_params_uses_expected_n_ctx`** with **`llama`** feature; **`--no-default-features`** suite stays green.
- **Changelog** Unreleased notes for the above.

## 4. Scope Drift

### Additions (built beyond the PRD)

*No material additions beyond PRD scope.* (Version bump to `0.2.0` and CHANGELOG lines were product/release hygiene, not this PRD’s feature list.)

### Deferrals (planned but deferred)

- **Background warmup + spinner (PRD D-1 / OQ-3, tasks 1.5 / AC-1 for “overlapping” path):** **Deferred** — **Task 1.5** marked `[-]`. **Next:** a small follow-up plan or backlog item if product wants **non-blocking** load and a live “warming” indicator; see `decisions.md` **Future opportunities**.

- **Quantitative SC-1 / SC-2 (50% / 20% line-item improvements):** The PRD allows an **or** (e.g. first line not paying full load when blocking warmup is on, or **documented analysis** for steady-state). **In-repo** numbers were **not** committed; **`docs/performance-baseline.md`** gives the **process** and **analysis** stance. **Next:** run benchmarks per README and file results in team notes (OQ-4) if the business needs hard proof of the thresholds.

### Blockers (planned but blocked)

*None — no tasks marked `[!]`.*

## 5. Key Decisions

### Warmup default and surface (OQ-1)

**Context:** Product default for eager load vs low-memory / CI stability.  
**Decision:** Default **`off`**; only **`off`** and **`blocking`** in v1.  
**Impact:** Users opt in to startup cost; R-2 (OOM) mitigated; no `background` enum until 1.5 follow-up.  
See: `decisions.md` 2026-04-26 — Warmup default and modes (1.3 / OQ-1)

### Skip background warmup (1.5)

**Context:** PRD allowed blocking first; background adds threads and state.  
**Decision:** **Skip** 1.5; ship **blocking** + **“Loading local model…”** + banner.  
**Impact:** No cross-thread model sharing; simpler review and QG-5.  
See: `decisions.md` 2026-04-26 — Task 1.5 background warmup (skipped)

### `LlamaContext` lifetime (OQ-2, 2.1 / 2.2)

**Context:** Per-turn `new_context` cost vs `LlamaContext<'a>` borrowing `LlamaModel`.  
**Decision:** **Do not** keep `LlamaContext` on the session; **do** cache **`LlamaContextParams`**.  
**Impact:** True KV reuse is a **future** change (self-ref / upstream API).  
See: `decisions.md` 2026-04-26 — Long-lived `LlamaContext`

### Baseline numbers in-repo (2.3)

**Context:** NFR/SC need comparability; repo should not hardcode one machine.  
**Decision:** **Procedure + analysis** in `docs/performance-baseline.md`, not committed medians/p95.  
**Impact:** Team records **OQ-4** baseline externally.  
See: `decisions.md` 2026-04-26 — Task 2.3 numbers in-repo

**Minor decisions:** Documentation placement in README (`###` under Interactive); QG-5 notes (main thread only, no new threads) — `decisions.md` same date.

## 6. What Worked Well

- **Phased tasks (1.x docs/config/warmup, then 2.x engine/validation)** matched how risk was front-loaded: documentation and gates before llama path changes.
- **Figment + existing `CLAI_*` patterns** let new config land without a new dependency (C-2).
- **Explicit `decisions.md`** made retro synthesis fast and preserved OQ-1 / OQ-2 / 1.5 rationale.
- **`#[cfg(feature = "llama")]`** kept CI fast and NFR-5 honest.
- **Task 1.5 as optional** in the task list made skipping **`[-]`** a first-class outcome instead of a failed task.

## 7. What to Improve

- **Optional manual smoke (1.9)** is easy to **skip** in busy weeks; consider a **checklist in CI docs** or a **single scripted smoke** (still may need a GGUF on a runner).
- **SC-1/SC-2** are strong bars; the PRD’s **or** branch is easy to satisfy with **docs** but **product** may still want **one** recorded A/B on a reference machine—define **who owns OQ-4** and **when** to refresh numbers.
- **Long-lived `LlamaContext`** will need a **spike** or **upstream** issue before a big estimate; add a **spike task** in the next plan if p95 of second-line latency is still a pain point.

## 8. Open Items

*All top-level tasks are either **completed** `[x]` or **skipped** `[-]` with an explicit follow-up. There are no `[!]` or lingering `[ ]` tasks.*

| Task | Status | Next Step |
|------|--------|-----------|
| — | — | *None. Optional follow-up: new plan or backlog for **1.5** (background + spinner) and/or **context KV reuse** per `decisions.md`.* |

**Manual verification:** `decisions.md` still recommends a maintainer run **real GGUF** smoke for `off` vs `blocking`; that is **process**, not an open task ID.

## 9. Future Opportunities

From `decisions.md` and implementation:

- **Background warmup** and TTY **“warming”** indicator (completed task 1.5 scope).
- **Self-referential** session struct or **upstream** owned context for **KV cache reuse** between lines.
- **`LocalWarmupMode::Auto`** (e.g. TTY + model size heuristic) for OQ-1.
- Extract **common “phase verbose”** patterns if more local code paths get similar support logging.

---

*Retrospective prepared with the plan-retrospective workflow. Archive `plans/local-inference-warmup-and-latency/` to `plans/archive/` after stakeholder confirmation.*
