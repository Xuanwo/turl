# turl

`turl` is a Rust CLI and library for locating and reading local code-agent thread files.

## Features

- Supports URI input:
  - `codex://<session_id>`
  - `claude://<session_id>`
- Resolves thread files from local storage roots.
- Default output is markdown:
  - includes only `user` / `assistant` messages
  - filters tool-call related records
- `--raw` outputs original JSONL content.

## CLI

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592 --raw
```

## Install from PyPI

```bash
pip install xuanwo-turl
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592
```

PyPI package name is `xuanwo-turl`, and installed CLI command remains `turl`.

## Environment Variables

- `CODEX_HOME`: official Codex home directory
- Codex default root: `~/.codex`

- `CLAUDE_CONFIG_DIR`: official Claude Code config/data directory
- Claude default root: `~/.claude`

Resolution precedence:

- Codex: `CODEX_HOME` > `~/.codex`
- Claude: `CLAUDE_CONFIG_DIR` > `~/.claude`

## URI Rules

- URI format must be `<scheme>://<session_id>`
- Supported schemes: `codex`, `claude`
- Only session IDs are accepted (UUID-like format)

## Exit Behavior

- success: exit code `0`
- failure (not found, empty/unreadable file, invalid URI, etc.): non-zero exit code and error message on `stderr`

## Project Layout

- `turl-core`: URI parsing, provider resolvers, reading, rendering
- `turl-cli`: CLI wrapper around `turl-core`

## Current Scope

- local filesystem only
- providers: Codex and Claude
- no remote fetching
