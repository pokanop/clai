# Tasks: Interactive command history

## Overview

| Metric        | Count |
| ------------- | ----- |
| Total         | 8     |
| Completed     | 8     |
| In progress   | 0     |
| Not started   | 0     |
| Blocked       | 0     |

## Phase 1: Core implementation

- [x] **TTY line editor + stdin fallback** `[P0]` `[L]`
  - **Acceptance:** On TTY stdin+stdout, interactive loop uses `rustyline` with `clai>` prompt (styled when `out_style()`); non-TTY unchanged; init failure logs and falls back without panic.
  - **Notes**
  - **Completed**: 2026-04-27. `TtyInteractiveLineEditor` + dual read path in `session.rs`.

- [x] **In-memory history policy** `[P0]` `[M]`
  - **Acceptance:** `InteractiveHistoryStore`: qualifying lines only; consecutive dedup; max entry count (config default 1000, min 100); ~4 MiB char budget eviction; unit tests.
  - **Notes**
  - **Completed**: 2026-04-27. `src/interactive_history.rs` + `RecordQualifyingLineOnDrop` on model path.

- [x] **Config + env** `[P0]` `[S]`
  - **Acceptance:** `[interactive].history_max_entries` and `CLAI_INTERACTIVE__HISTORY_MAX_ENTRIES`; documented; test parse.
  - **Notes**
  - **Completed**: 2026-04-27.

- [x] **Document help + README** `[P0]` `[S]`
  - **Acceptance:** Session `help` and README describe TTY vs pipe, builtins excluded, cap, privacy caveat.
  - **Notes**
  - **Completed**: 2026-04-27.

## Phase 2: Verify

- [x] **Quality gates** `[P0]` `[S]`
  - **Acceptance:** `cargo fmt --check`, `cargo test --no-default-features --locked`, `cargo clippy --no-default-features --locked -D warnings`, `cargo build --locked`.
  - **Notes**
  - **Completed**: 2026-04-27.

---

`tasks.md` was created at implementation time (no prior `prd-to-tasks` artifact in-repo).
