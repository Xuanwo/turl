# turl

`turl` is a Rust CLI and library for locating and reading local code-agent thread files.

## Features

- Multi-agent thread resolution:
  - <img src="https://ampcode.com/amp-mark-color.svg" alt="Amp logo" width="16" height="16" /> Amp
  - <img src="https://avatars.githubusercontent.com/u/14957082?s=24&v=4" alt="Codex logo" width="16" height="16" /> Codex
  - <img src="https://www.anthropic.com/favicon.ico" alt="Claude logo" width="16" height="16" /> Claude
  - <img src="https://www.google.com/favicon.ico" alt="Gemini logo" width="16" height="16" /> Gemini
  - <img src="https://opencode.ai/favicon.ico" alt="OpenCode logo" width="16" height="16" /> OpenCode
- Default output is timeline markdown with user/assistant messages and compact markers.
- `--raw` outputs raw thread records.
- Automatically respects official environment variables and default local data roots for each supported agent.

## Install

```bash
npx skills add Xuanwo/turl
```

## Agents

### Amp

- Supported URI:
  - `amp://<thread_id>`
- Thread id format:
  - `T-xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Resolution:
  - `XDG_DATA_HOME/amp/threads/<thread_id>.json`
  - fallback: `~/.local/share/amp/threads/<thread_id>.json`
- Example:

```bash
turl amp://T-019c0797-c402-7389-bd80-d785c98df295
```

### Codex

- Supported URIs:
  - `codex://<session_id>`
  - `codex://threads/<session_id>`
- Resolution order:
  - SQLite thread index under `CODEX_HOME` (`state_<version>.sqlite` first, then `state.sqlite`) via `threads(id, rollout_path, archived)`.
  - Filesystem fallback under `sessions/` and `archived_sessions/` for `rollout-*.jsonl`.
- Examples:

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592
turl codex://threads/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

### Claude

- Supported URI:
  - `claude://<session_id>`
- Example:

```bash
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f
```

### OpenCode

- Supported URI:
  - `opencode://<session_id>`
- Example:

```bash
turl opencode://ses_43a90e3adffejRgrTdlJa48CtE
```

### Gemini

- Supported URI:
  - `gemini://<session_id>`
- Session id format:
  - `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Resolution:
  - `GEMINI_CLI_HOME/.gemini/tmp/*/chats/session-*.json`
  - fallback: `~/.gemini/tmp/*/chats/session-*.json`
- Example:

```bash
turl gemini://29d207db-ca7e-40ba-87f7-e14c9de60613
```
