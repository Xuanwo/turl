---
name: xurl
description: Use the xurl CLI to resolve unified agents:// URIs (and legacy provider URIs) for Amp, Codex, Claude, Gemini, Pi, and OpenCode thread reading workflows.
---

# xurl

Use this skill when you need to read AI agent thread content by URI.

## Installation

Install `xurl` from package `xuanwo-xurl` via `uv`:

```bash
uv tool install xuanwo-xurl
xurl --version
```

## When to Use

- The user gives an `agents://...` URI for `amp`, `codex`, `claude`, `gemini`, `pi`, or `opencode`.
- The user gives legacy URIs like `codex://...`, `claude://...`, `pi://...`, `amp://...`, `gemini://...`, or `opencode://...`.
- The user asks to inspect, view, or fetch thread content.
- You need to quote or reuse prior context in workflows like compact, handoff, or delegate.
- You need to find subagent or branch targets before drilling into a specific child thread.

## URI Construction Playbook

1. Identify provider and id source.
- Provider usually comes from context (`codex`, `claude`, `amp`, `gemini`, `pi`, `opencode`).
- Prefer ids copied from existing links, list output, or known session metadata.

2. Build the canonical URI.
- Main thread:
  - `agents://codex/<session_id>` (or deep-link `agents://codex/threads/<session_id>`)
  - `agents://claude/<session_id>`
  - `agents://amp/<thread_id>`
  - `agents://gemini/<session_id>`
  - `agents://pi/<session_id>`
  - `agents://opencode/<session_id>`
- Child target:
  - `agents://codex/<main_session_id>/<agent_id>`
  - `agents://claude/<main_session_id>/<agent_id>`
  - `agents://pi/<session_id>/<entry_id>`

3. Validate mode constraints.
- `--list` must be used with a main thread URI, not with a child URI.
- `amp`, `gemini`, and `opencode` do not support child path segments.

4. If child id is unknown, discover first.
- Use `xurl <main_uri> --list` to get valid child targets (Codex/Claude subagents, Pi entries).
- Copy URI/id from the list output instead of guessing.

## Supported URI Forms

Canonical:

- `agents://codex/<session_id>`
- `agents://codex/threads/<session_id>`
- `agents://codex/<main_session_id>/<agent_id>`
- `agents://amp/<thread_id>`
- `agents://claude/<session_id>`
- `agents://claude/<main_session_id>/<agent_id>`
- `agents://gemini/<session_id>`
- `agents://pi/<session_id>`
- `agents://pi/<session_id>/<entry_id>`
- `agents://opencode/<session_id>`

Legacy compatibility:

- `codex://<session_id>`
- `codex://threads/<session_id>`
- `codex://<main_session_id>/<agent_id>`
- `amp://<thread_id>`
- `claude://<session_id>`
- `claude://<main_session_id>/<agent_id>`
- `gemini://<session_id>`
- `pi://<session_id>`
- `pi://<session_id>/<entry_id>`
- `opencode://<session_id>`

## Input-to-URI Examples

- Provider + main id:
  - input: `provider=codex`, `session_id=019c871c-b1f9-7f60-9c4f-87ed09f13592`
  - uri: `agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592`
- Codex deep-link from UI:
  - input: `codex://threads/019c871c-b1f9-7f60-9c4f-87ed09f13592`
  - uri: `agents://codex/threads/019c871c-b1f9-7f60-9c4f-87ed09f13592`
- Main uri + child id:
  - input: `agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f` + `acompact-69d537`
  - uri: `agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537`
- Pi branch drill-down:
  - input: `agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f --list` output entry `d1b2c3d4`
  - uri: `agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4`

## Commands

Default output (timeline markdown with user/assistant messages and compact markers):

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Raw JSONL output:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 --raw
```

Discover child targets first:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 --list
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f --list
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f --list
```

Codex subagent drill-down:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

Claude thread and subagent examples:

```bash
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
```

Codex deep-link example:

```bash
xurl agents://codex/threads/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Other providers:

```bash
xurl agents://opencode/ses_43a90e3adffejRgrTdlJa48CtE
xurl agents://gemini/29d207db-ca7e-40ba-87f7-e14c9de60613
xurl agents://amp/T-019c0797-c402-7389-bd80-d785c98df295
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4
```

## Construction Examples for Common Agent Tasks

Compact (Claude child thread from known main + agent id):

```bash
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
```

Handoff (Codex deep-link shared by another agent):

```bash
xurl agents://codex/threads/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Delegate follow-up (discover child first, then drill down):

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 --list
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

## Agent Behavior

- Prefer canonical `agents://` URIs when constructing links or commands.
- Legacy provider schemes are accepted, so keep workflows compatible with existing links.
- If the user does not request `--raw`, use default markdown output first.
- If the user asks for subagent aggregation, use `--list` with the parent thread URI.
- If the user asks for Pi session navigation targets, use `--list` with `agents://pi/<session_id>`.
- If the user requests exact records, rerun with `--raw`.
- If the output is long, redirect to a temp file and grep/summarize based on the user request.
- Do not infer or reinterpret thread meaning unless the user explicitly asks for analysis.

## Failure Handling

- Common failures include invalid URI format, invalid mode combinations, and missing thread files.
- Typical invalid mode example: `--list` with `agents://<provider>/<main_thread_id>/<agent_id>`.
