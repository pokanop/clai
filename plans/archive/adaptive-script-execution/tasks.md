# Tasks: adaptive-script-execution

## Overview

| Phase | Total | Completed |
|-------|-------|-----------|
| Phase 1 — Tooling + prompt + doctor | 4 | 4 |
| Phase 2 — Ephemeral scripts + tests | 5 | 5 |

## Phase 1: Tooling and prompt

- [x] **1.1** Runtime PATH probes + process cache `[P0]` `[M]`
  - **Acceptance**: Structured snapshot (python/node/ruby/…); off when `detect_runtimes = false`; under `runtime_tooling` module.
  - **Completed**: 2026-04-26. `runtime_tooling.rs` + `OnceLock` cache.
- [x] **1.2** Config `[tooling]` + env overrides `[P0]` `[S]`
  - **Acceptance**: `detect_runtimes`, `ephemeral_scripts` (default false), `prefer_scripts_when_available`; serde defaults; no migration bump.
  - **Completed**: 2026-04-26. `ToolingConfig` on `AppConfig`; `CLAI_TOOLING__*` via figment.
- [x] **1.3** System prompt + cloud/local parity `[P0]` `[M]`
  - **Acceptance**: `build_system_prompt` includes tooling JSON + script guidance; ask + interactive use same builder.
  - **Completed**: 2026-04-26. `main.rs` builder; cloud uses same `system` string as local `ask`.
- [x] **1.4** `clai doctor` lists tool categories `[P0]` `[S]`
  - **Acceptance**: When detection on, each category shows present/absent (or path); when off, says disabled.
  - **Completed**: 2026-04-26. `print_doctor_report` "Runtime tooling" section.

## Phase 2: Ephemeral scripts

- [x] **2.1** Schema `script_body` + `script_extension` `[P0]` `[S]`
  - **Acceptance**: Parse/serialize; `schema_json` updated for cloud strict schema.
  - **Completed**: 2026-04-26. `CommandProposal` + unit test; cloud uses `schema_json()`.
- [x] **2.2** Materialize + RAII cleanup (success/fail/early exit) `[P0]` `[L]`
  - **Acceptance**: Temp dir user-private perms where supported; cleanup before `process::exit`; error if `script_body` when ephemeral disabled.
  - **Completed**: 2026-04-26. `ephemeral_script.rs` + `discard_ephemeral_temp` in `cmd_ask`.
- [x] **2.3** Policy + pre-run + verbose hints `[P0]` `[M]`
  - **Acceptance**: Policy sees final argv; pre-run shows managed script note; verbose logs path pattern/count.
  - **Completed**: 2026-04-26. `PreRunLine::ManagedTempScript`; verbose one-file note.
- [x] **2.4** Session loop integration `[P0]` `[M]`
  - **Acceptance**: Interactive path matches ask materialization behavior.
  - **Completed**: 2026-04-26. `session.rs` prepare + `_ephemeral_script_hold`.
- [x] **2.5** Automated tests + golden scenarios `[P0]` `[M]`
  - **Acceptance**: Unit tests probes/cleanup; policy regression; ≥5 golden scenario strings documented in tests.
  - **Completed**: 2026-04-26. `tests/adaptive_script_golden.rs`, policy/ schema/ config/ ephemeral tests.
