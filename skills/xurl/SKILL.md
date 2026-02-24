---
name: xurl
description: Use xurl to read, discover, and write AI agent conversations through agents:// URIs.
---

## When to Use

- User gives `agents://...` URI.
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

### 1) Read

```bash
xurl agents://codex/<conversation_id>
```

### 2) Discover

```bash
xurl -I agents://codex/<conversation_id>
```

Use returned `subagents` or `entries` URI for next step.
OpenCode child linkage is validated by sqlite `session.parent_id`.

### 2.1) Drill Down Child Thread

```bash
xurl agents://codex/<main_conversation_id>/<agent_id>
```

### 3) Write

Create:

```bash
xurl agents://codex -d "Start a new conversation"
```

Append:

```bash
xurl agents://codex/<conversation_id> -d "Continue"
```

Payload from file/stdin:

```bash
xurl agents://codex -d @prompt.txt
cat prompt.md | xurl agents://claude -d @-
```

## Command Rules

- Base form: `xurl [OPTIONS] <URI>`
- `-I, --head`: frontmatter/discovery only
- `-d, --data`: write payload, repeatable
- `-o, --output`: write command output to file

Write mode rules:

- `agents://<provider> -d ...` => create
- `agents://<provider>/<conversation_id> -d ...` => append
- child URI write is rejected
- `--head` and `--data` cannot be combined
- multiple `-d` values are newline-joined

Write output:

- assistant text: `stdout` (or `--output` file)
- canonical URI: `stderr` as `created: ...` / `updated: ...`

## URI Formats

Canonical:

- `agents://<provider>/<conversation_id>`
- `agents://<provider>/<conversation_id>/<child_id>`

Child drill-down URI forms:

- `agents://<provider>/<main_conversation_id>/<child_id>`

Legacy compatibility:

- `<provider>://<conversation_id>`
- `<provider>://<conversation_id>/<child_id>`

Pi child forms:

- `agents://pi/<session_id>/<child_id>`: `<child_id>` can be a UUID child session id or an entry id

## Failure Handling

### `command not found: <agent>`

search web to install the `<agent>` `CLI, ask users to authenticate the agent
