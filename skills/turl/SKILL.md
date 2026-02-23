---
name: turl
description: Use the turl CLI to resolve Amp, Codex, Claude, Gemini, or OpenCode thread URIs and print thread content in markdown or raw records.
---

# turl

Use this skill when you need to read a thread file by URI.

## Installation

Install `turl` from package `xuanwo-turl` via `uv`:

```bash
uv tool install xuanwo-turl
turl --version
```

## When to Use

- The user gives an `amp://...`, `codex://...`, `codex://threads/...`, `claude://...`, `gemini://...`, or `opencode://...` URI.
- The user asks to inspect, view, or fetch thread content.

## Input

- A thread URI in one of these forms:
  - `codex://<session_id>`
  - `codex://threads/<session_id>`
  - `codex://<main_session_id>/<agent_id>`
  - `amp://<thread_id>`
  - `claude://<session_id>`
  - `claude://<main_session_id>/<agent_id>`
  - `gemini://<session_id>`
  - `opencode://<session_id>`

## Commands

Default output (timeline markdown with user/assistant messages and compact markers):

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Raw JSONL output:

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592 --raw
```

Codex subagent aggregate view:

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592 --list
```

Codex subagent drill-down:

```bash
turl codex://019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

Claude thread example:

```bash
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f
```

Claude subagent aggregate view:

```bash
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f --list
```

Claude subagent drill-down:

```bash
turl claude://2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
```

Codex deep-link example:

```bash
turl codex://threads/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

OpenCode thread example:

```bash
turl opencode://ses_43a90e3adffejRgrTdlJa48CtE
```

Gemini thread example:

```bash
turl gemini://29d207db-ca7e-40ba-87f7-e14c9de60613
```

Amp thread example:

```bash
turl amp://T-019c0797-c402-7389-bd80-d785c98df295
```

## Agent Behavior

- If the user does not request `--raw`, use default markdown output first.
- If the user asks for subagent aggregation, use `--list` with the parent thread URI.
- If the user requests exact records, rerun with `--raw`.
- Return the command output directly.
- Do not infer or reinterpret thread meaning unless the user explicitly asks for analysis.
- The output could be long, redirect to temp files and read/grep it later.

## Failure Handling

- Only fatal failures are emitted on `stderr`; non-fatal diagnostics are not printed.
- Common failures include invalid URI format and missing thread files.
