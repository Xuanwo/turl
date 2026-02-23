# xURL

`xURL` is a client for AI agent URLs.

> Also known as **Xuanwo's URL**.

## Features

- Multi-agent thread resolution:
  - <img src="https://ampcode.com/amp-mark-color.svg" alt="Amp logo" width="16" height="16" /> Amp
  - <img src="https://avatars.githubusercontent.com/u/14957082?s=24&v=4" alt="Codex logo" width="16" height="16" /> Codex
  - <img src="https://www.anthropic.com/favicon.ico" alt="Claude logo" width="16" height="16" /> Claude
  - <img src="https://www.google.com/favicon.ico" alt="Gemini logo" width="16" height="16" /> Gemini
  - <img src=".github/assets/pi-logo-dark.svg" alt="Pi logo" width="16" height="16" /> Pi
  - <img src="https://opencode.ai/favicon.ico" alt="OpenCode logo" width="16" height="16" /> OpenCode
- Unified URI scheme: `agents://<provider>/<thread_path>` is the primary format.
- Legacy provider URIs (`codex://...`, `claude://...`, etc.) are still accepted for compatibility.
- Default output is timeline markdown with user/assistant messages and compact markers.
- `--raw` outputs raw thread records.
- `--list` outputs subagent status aggregation for providers that support subagent transcripts.
- Subagent markdown views print full parent/subagent URIs in `agents://...` format.
- Non-fatal diagnostics are kept internal; only fatal errors are printed to `stderr`.
- Automatically respects official environment variables and default local data roots for each supported agent.

## Install

Install from npm and run directly with `npx`:

```bash
npx @xuanwo/xurl --help
```

Or install globally via npm:

```bash
npm install -g @xuanwo/xurl
xurl --help
```

Install as a Codex skill:

```bash
npx skills add Xuanwo/xurl
```

## URL Format

Primary URI format:

```text
agents://<provider>/<thread_path>
```

ASCII breakdown:

```text
agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
^^^^^^   ^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
scheme   provider thread_path (provider-specific: main thread, optional child thread)
```

## Agents

### Amp

- Supported URIs:
  - `agents://amp/<thread_id>`
  - legacy: `amp://<thread_id>`
- Thread id format:
  - `T-xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Resolution:
  - `XDG_DATA_HOME/amp/threads/<thread_id>.json`
  - fallback: `~/.local/share/amp/threads/<thread_id>.json`
- Example:

```bash
xurl agents://amp/T-019c0797-c402-7389-bd80-d785c98df295
```

### Codex

- Supported URIs:
  - `agents://codex/<session_id>`
  - `agents://codex/threads/<session_id>`
  - `agents://codex/<main_session_id>/<agent_id>`
  - legacy: `codex://<session_id>`, `codex://threads/<session_id>`, `codex://<main_session_id>/<agent_id>`
- Subagent modes:
  - Aggregate: `xurl agents://codex/<main_session_id> --list`
  - Drill-down: `xurl agents://codex/<main_session_id>/<agent_id>`
- Resolution order:
  - SQLite thread index under `CODEX_HOME` (`state_<version>.sqlite` first, then `state.sqlite`) via `threads(id, rollout_path, archived)`.
  - Filesystem fallback under `sessions/` and `archived_sessions/` for `rollout-*.jsonl`.
- Examples:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
xurl agents://codex/threads/019c871c-b1f9-7f60-9c4f-87ed09f13592
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 --list
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

### Claude

- Supported URIs:
  - `agents://claude/<session_id>`
  - `agents://claude/<main_session_id>/<agent_id>`
  - legacy: `claude://<session_id>`, `claude://<main_session_id>/<agent_id>`
- Subagent modes:
  - Aggregate: `xurl agents://claude/<main_session_id> --list`
  - Drill-down: `xurl agents://claude/<main_session_id>/<agent_id>`
- Example:

```bash
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f --list
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
```

### OpenCode

- Supported URIs:
  - `agents://opencode/<session_id>`
  - legacy: `opencode://<session_id>`
- Example:

```bash
xurl agents://opencode/ses_43a90e3adffejRgrTdlJa48CtE
```

### Gemini

- Supported URI:
  - `agents://gemini/<session_id>`
  - legacy: `gemini://<session_id>`
- Session id format:
  - `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Resolution:
  - `GEMINI_CLI_HOME/.gemini/tmp/*/chats/session-*.json`
  - fallback: `~/.gemini/tmp/*/chats/session-*.json`
- Example:

```bash
xurl agents://gemini/29d207db-ca7e-40ba-87f7-e14c9de60613
```

### Pi

- Supported URIs:
  - `agents://pi/<session_id>`
  - `agents://pi/<session_id>/<entry_id>`
  - legacy: `pi://<session_id>`, `pi://<session_id>/<entry_id>`
- Session id format:
  - `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Resolution:
  - `PI_CODING_AGENT_DIR/sessions/**/*.jsonl`
  - fallback: `~/.pi/agent/sessions/**/*.jsonl`
- Rendering:
  - `agents://pi/<session_id>` renders the latest leaf branch in the session tree.
  - `agents://pi/<session_id>/<entry_id>` renders the branch ending at the specified entry id.
  - `agents://pi/<session_id> --list` lists all entries and marks leaf entries that are good drill-down targets.
- Example:

```bash
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f --list
```
