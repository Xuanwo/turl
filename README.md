# turl

`turl` is a Rust CLI and library for locating and reading local code-agent thread files.

## Features

- Multi-agent thread resolution:
  - <img src="https://ampcode.com/amp-mark-color.svg" alt="Amp logo" width="16" height="16" /> Amp
  - <img src="https://avatars.githubusercontent.com/u/14957082?s=24&v=4" alt="Codex logo" width="16" height="16" /> Codex
  - <img src="https://www.anthropic.com/favicon.ico" alt="Claude logo" width="16" height="16" /> Claude
  - <img src="https://www.google.com/favicon.ico" alt="Gemini logo" width="16" height="16" /> Gemini
  - <img src=".github/assets/pi-logo-dark.svg" alt="Pi logo" width="16" height="16" /> Pi
  - <img src="https://opencode.ai/favicon.ico" alt="OpenCode logo" width="16" height="16" /> OpenCode
- Default output is timeline markdown with user/assistant messages and compact markers.
- `--raw` outputs raw thread records.
- `--list` outputs subagent status aggregation for providers that support subagent transcripts.
  - For Pi, `--list` outputs session entries with leaf markers and entry URIs.
- Subagent markdown views always print full parent/subagent URIs (`<provider>://<main>` and `<provider>://<main>/<agent>`).
- Non-fatal diagnostics are kept internal; only fatal errors are printed to `stderr`.
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
  - `codex://<main_session_id>/<agent_id>`
- Subagent modes:
  - Aggregate: `turl codex://<main_session_id> --list`
  - Drill-down: `turl codex://<main_session_id>/<agent_id>`
- Resolution order:
  - SQLite thread index under `CODEX_HOME` (`state_<version>.sqlite` first, then `state.sqlite`) via `threads(id, rollout_path, archived)`.
  - Filesystem fallback under `sessions/` and `archived_sessions/` for `rollout-*.jsonl`.
- Examples:

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592
turl codex://threads/019c871c-b1f9-7f60-9c4f-87ed09f13592
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592 --list
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

### Claude

- Supported URIs:
  - `claude://<session_id>`
  - `claude://<main_session_id>/<agent_id>`
- Subagent modes:
  - Aggregate: `turl claude://<main_session_id> --list`
  - Drill-down: `turl claude://<main_session_id>/<agent_id>`
- Example:

```bash
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f --list
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
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

### Pi

- Supported URIs:
  - `pi://<session_id>`
  - `pi://<session_id>/<entry_id>`
- Session id format:
  - `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Resolution:
  - `PI_CODING_AGENT_DIR/sessions/**/*.jsonl`
  - fallback: `~/.pi/agent/sessions/**/*.jsonl`
- Rendering:
  - `pi://<session_id>` renders the latest leaf branch in the session tree.
  - `pi://<session_id>/<entry_id>` renders the branch ending at the specified entry id.
  - `pi://<session_id> --list` lists all entries and marks leaf entries that are good drill-down targets.
- Example:

```bash
turl pi://12cb4c19-2774-4de4-a0d0-9fa32fbae29f
turl pi://12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4
turl pi://12cb4c19-2774-4de4-a0d0-9fa32fbae29f --list
```
