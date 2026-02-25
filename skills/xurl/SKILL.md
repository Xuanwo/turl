---
name: xurl
description: Use xurl to read, discover, and write AI agent conversations through agents:// URIs.
---

## When to Use

- User gives `agents://...` URI.
- User asks to list/search provider threads.
- User asks to read or summarize a conversation.
- User asks to discover child targets before drill-down.
- User asks to start or continue conversations for providers.

## Installation

Pick up the preferred ways based on current context:

### Homebrew

Install via Homebrew tap:

```bash
brew tap xuanwo/tap
brew install xurl
xurl --version
```

Upgrade via Homebrew:

```bash
brew update
brew upgrade xurl
```

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

## Core Workflows

### 1) Query

List latest provider threads:

```bash
xurl agents://codex
```

Keyword query with optional limit (default `10`):

```bash
xurl 'agents://codex?q=spawn_agent'
xurl 'agents://claude?q=agent&limit=5'
```

### 2) Read

```bash
xurl agents://codex/<conversation_id>
```

### 3) Discover

```bash
xurl -I agents://codex/<conversation_id>
```

Use returned `subagents` or `entries` URI for next step.
OpenCode child linkage is validated by sqlite `session.parent_id`.

### 3.1) Drill Down Child Thread

```bash
xurl agents://codex/<main_conversation_id>/<agent_id>
```

### 4) Write

Create:

```bash
xurl agents://codex -d "Start a new conversation"
```

Append:

```bash
xurl agents://codex/<conversation_id> -d "Continue"
```

Create with query parameters:

```bash
xurl "agents://codex?workdir=%2FUsers%2Falice%2Frepo&add_dir=%2FUsers%2Falice%2Fshared&model=gpt-5" -d "Review this patch"
```

Payload from file/stdin:

```bash
xurl agents://codex -d @prompt.txt
cat prompt.md | xurl agents://claude -d @-
```

## Command Reference

- Base form: `xurl [OPTIONS] <URI>`
- `-I, --head`: frontmatter/discovery only
- `-d, --data`: write payload, repeatable
- `-o, --output`: write command output to file

Mode rules:

- child URI write is rejected
- `--head` and `--data` cannot be combined
- multiple `-d` values are newline-joined

Write output:

- assistant text: `stdout` (or `--output` file)
- canonical URI: `stderr` as `created: ...` / `updated: ...`

## URI Reference

Read/discovery URIs:

- `agents://<provider>?q=<keyword>&limit=<n>`
- `agents://<provider>/<conversation_id>`
- `agents://<provider>/<conversation_id>/<child_id>`

Write URIs:

- `agents://<provider>?k=v` (create)
- `agents://<provider>/<conversation_id>` (append)

Read query keys:

- `q`: keyword search only
- `limit`: max result count, default `10`

Create query keys:

- standard: `workdir`, `add_dir` (repeatable)
- unknown keys are passthrough (`k=v` -> `--k v`, `k` -> `--k`)
- repeated keys preserve URI order
- reserved conflicting keys are ignored with `warning:` on stderr
- `workdir` must be non-empty and directory-valid
- empty `add_dir` is ignored with warning
- append mode ignores all URI query keys with warnings

Provider mapping:

- `workdir`: always effective by process `cwd`; Codex also gets `--cd`, OpenCode also gets `--dir`
- `add_dir`: Codex `--add-dir`, Claude `--add-dir`, Gemini `--include-directories`
- `add_dir`: ignored with warning for Amp, Pi, OpenCode

Child drill-down URI forms:

- `agents://<provider>/<main_conversation_id>/<child_id>`

Legacy compatibility:

- `<provider>://<conversation_id>`
- `<provider>://<conversation_id>/<child_id>`

Pi child forms:

- `agents://pi/<session_id>/<child_id>`: `<child_id>` can be a UUID child session id or an entry id

## Failure Handling

### `command not found: <agent>`

Install the provider CLI, then complete provider authentication before retrying.
