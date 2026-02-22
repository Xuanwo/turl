# Agent Guidelines

## Workspace Responsibilities
- `turl-core`: all URI parsing, provider resolution, raw file reading, and markdown rendering lives here. It owns provider-specific parsers for Codex and Claude plus the shared service layer (`resolve_thread`, `read_thread_raw`, `render_thread_markdown`).
- `turl-cli`: thin CLI that parses `turl <uri>` arguments with `clap`, wires up `ProviderRoots::from_env_or_home`, emits metadata warnings to `stderr`, and prints either raw JSON (via `read_thread_raw`) or rendered markdown (`render_thread_markdown`).

## CLI Parameter & Provider Behavior Matrix
- The CLI accepts a single `<uri>` (scheme must be `codex` or `claude`) and an optional `--raw` flag.
- `ProviderRoots::from_env_or_home` sources the base directories using `CODEX_HOME`/`~/.codex` and `CLAUDE_CONFIG_DIR`/`~/.claude` per README precedence rules, then the appropriate provider resolves `<session_id>.jsonl` from either branch:
  - raw mode: `read_thread_raw` simply dumps the JSONL file contents with no filtering; applicable to both Codex and Claude threads without branch-specific formatting.
  - rendered mode: `render_thread_markdown` converts Codex or Claude payloads to a markdown log via provider-aware extraction logic before printing; warnings from `resolved.metadata.warnings` are emitted to `stderr` before the primary output.

## Error Handling & Exit Contract
- `main` maps any `turl_core::Result` failure to a non-zero exit code (1) and prints `error: <message>` on `stderr`; successes return exit code 0.
- Common failure cases include invalid URI syntax, missing provider roots, unresolved session IDs, unreadable files, empty files, and non-UTF-8 payloads (`read_thread_raw` explicitly guards against empty and non-UTF8 data).
- Metadata warnings and diagnostics are printed on `stderr` but do not change the exit code, making it clear that only `Err` results trigger failure.

## Style, Lint & Test Constraints
- The workspace enforces `cargo clippy` with `all = warn` and `pedantic = warn` at the root (`Cargo.toml` workspace lints); follow Rust formatting conventions and keep identifiers/comments in English.
- Tests are scoped to the crates (`turl-cli/tests/cli.rs` for argument coverage and raw output, `turl-core` unit tests for file-reading edge cases). Run `cargo test` when touching parsing, rendering, or CLI behaviors.

## Minimal Change Strategy for Agents
- Keep patches as small as possible: touch only the crate that owns the behavior, avoid cross-crate refactors unless the fix explicitly requires both `turl-core` and `turl-cli`.
- Unless the user asks for new optional behavior, do not add new dependencies, features, or files; if you see adjacent concerns, surface them as follow-up items instead of folding them into the current change.
- Document any deviation from the existing light-touch approach (e.g., introducing new public interfaces or broader renders) so reviewers know why the scope expanded.
