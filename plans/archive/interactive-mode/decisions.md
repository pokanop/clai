# Implementation decisions: interactive mode

## 2026-04-26

- **Config version:** Left `CONFIG_VERSION_LATEST` at `1`. New `[interactive]` table uses `serde(default)` so existing configs deserialize without a migration step (FR-19).
- **Styling dependency:** Used manual ANSI escapes in `src/tty.rs` (MIT-licensed project code) instead of adding `anstream` / `owo-colors` to keep the dependency graph unchanged; `NO_COLOR` and `IsTerminal` gate output.
- **`clai doctor`:** Prints effective interactive mode from **config + env** only; CLI overrides (`--interactive-mode`, `--yes`) are not applied because `doctor` does not take those flags—documented in the doctor line.
- **Cloud HTTP:** Reused stateless `complete_cloud` per line; no `ureq` agent pooling (best-effort parity; same as one-shot `ask`).
- **Session child exit:** After a run, the session stays open and reports `clai_ask_process_exit` mapping as a warning if non-zero; the **process** does not exit with the child code (unlike one-shot `clai ask`), so users can continue the loop.
- **FR-16 dry-run:** Interactive `dry-run` skips policy sensitive confirmation and execution prompts (steps 2–4), matching PRD §FR-16.

## Future opportunities

- Optional `ureq::Agent` or client reuse for cloud sessions if latency becomes an issue.
- Richer signal handling for Ctrl+C during `read_line` / model generation.
