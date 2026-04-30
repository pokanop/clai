# Decisions: adaptive-script-execution

## 2026-04-26

- **Runtime struct**: Introduced `RuntimeTooling` in `src/runtime_tooling.rs` (per PRD D-1) instead of extending `HostContext`, keeping host facts and PATH probes separate and JSON-stable.
- **Process cache**: `OnceLock` caches probe results when `detect_runtimes` is true (NFR-1); when false, no PATH walks and an empty snapshot is returned.
- **Config defaults**: `detect_runtimes = true`, `ephemeral_scripts = false`, `prefer_scripts_when_available = true` (ephemeral off until explicitly enabled per PRD §9).
- **Docker + `script_body`**: Rejected with a clear error — container mount scope does not include host temp files (FR-5 safe subset).
- **Temp files**: Promoted `tempfile` to a normal dependency for managed dirs; Unix uses `OpenOptionsExt::mode(0o600)` (NFR-3).
- **`process::exit` vs cleanup**: `cmd_ask` calls `discard_ephemeral_temp` before every `process::exit` and on policy block after materialization so temp dirs are removed even when `Drop` would not run.
- **Policy**: Evaluates the materialized proposal (interpreter + args including temp path); added regression test for strict allowlist + final argv (FR-7).

## Future opportunities

- Bwrap + ephemeral scripts: not special-cased; full root bind may make `/tmp` scripts visible — revisit if users report surprises.
- Windows: PATH probing uses `is_file` heuristic without PATHEXT enumeration on the bare name (still tries `.exe`/`.bat`/`.cmd`).
