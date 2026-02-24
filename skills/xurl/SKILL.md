---
name: xurl
description: Use the xurl CLI to resolve unified agents:// URIs (and legacy provider URIs) for Amp, Codex, Claude, Gemini, Pi, and OpenCode thread reading workflows.
---

# xurl

Use this skill when you need to read AI agent thread content by URI.

## Installation

Pick up the preferred ways based on current context:

### Python Env

install from PyPI via `uv`:

```bash
uv tool install xuanwo-xurl
xurl --version
```

Upgrade `xurl` installed by `uv`:

```bash
uv tool upgrade xuanwo-xurl
xurl --version
```

### Node Env

Temporary usage without install:

```bash
npx @xuanwo/xurl --help
```

install globally via npm:

```bash
npm install -g @xuanwo/xurl
xurl --version
```

Upgrade `xurl` installed by npm:

```bash
npm update -g @xuanwo/xurl
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
- Prefer ids copied from existing links, head output, or known session metadata.

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
- `--head` can be used with both main and child URIs.
- `amp`, `gemini`, and `opencode` do not support child path segments.

4. If child id is unknown, discover first.
- Use `xurl -I <main_uri>` to get valid child targets (Codex/Claude `subagents`, Pi `entries`).
- Copy URI/id from the frontmatter output instead of guessing.

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
  - input: `xurl -I agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f` output entry `d1b2c3d4`
  - uri: `agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4`

## Commands

Default output (timeline markdown with frontmatter and timeline entries):

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Frontmatter includes machine-readable source metadata:

```bash
# output starts with:
# ---
# uri: 'agents://codex/...'
# thread_source: '/abs/path/to/thread.jsonl'
# ---
```

Discover child targets first (head-only output):

```bash
xurl -I agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
xurl -I agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f
xurl -I agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f
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
xurl -I agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

## Agent Behavior

- Prefer canonical `agents://` URIs when constructing links or commands.
- Legacy provider schemes are accepted, so keep workflows compatible with existing links.
- Use default markdown output and read frontmatter (`thread_source`) when raw file access is needed.
- If the user asks for subagent aggregation, use `-I/--head` with the parent thread URI and read `subagents`.
- If the user asks for Pi session navigation targets, use `-I/--head` with `agents://pi/<session_id>` and read `entries`.
- If the user requests exact records, read the `thread_source` path from frontmatter.
- If the output is long, redirect to a temp file and grep/summarize based on the user request.
- Do not infer or reinterpret thread meaning unless the user explicitly asks for analysis.

## Failure Handling

- Common failures include invalid URI format, invalid mode combinations, and missing thread files.
- Typical invalid flag example: `--list` is no longer supported; use `-I/--head`.
