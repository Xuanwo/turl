---
name: turl
description: Use the turl CLI to resolve a Codex or Claude thread URI and print thread content in markdown or raw JSONL.
---

# turl

Use this skill when you need to read a thread file by URI.

## When to Use

- The user gives a `codex://...` or `claude://...` URI.
- The user asks to inspect, view, or fetch thread content.

## Input

- A thread URI in one of these forms:
  - `codex://<session_id>`
  - `claude://<session_id>`

## Commands

Default output (filtered markdown with user/assistant messages):

```bash
turl "codex://019c871c-b1f9-7f60-9c4f-87ed09f13592"
```

Raw JSONL output:

```bash
turl "codex://019c871c-b1f9-7f60-9c4f-87ed09f13592" --raw
```

Claude thread example:

```bash
turl "claude://2823d1df-720a-4c31-ac55-ae8ba726721f"
```

## Agent Behavior

- If the user does not request `--raw`, use default markdown output first.
- If the user requests exact records, rerun with `--raw`.
- Return the command output directly.
- Do not infer or reinterpret thread meaning unless the user explicitly asks for analysis.

## Failure Handling

- Surface `turl` stderr as-is.
- Common failures include invalid URI format and missing thread files.
