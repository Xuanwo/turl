# xURL

`xURL` is a client for AI agent URLs.

> Also known as **Xuanwo's URL**.

## What xURL Can Do

- Read an agent conversation as markdown.
- Query recent threads and keyword matches for a provider.
- Discover subagent/branch navigation targets.
- Start a new conversation with agents.
- Continue an existing conversation with follow-up prompts.

## Quick Start

1. Add `xurl` as an agent skill:

```bash
npx skills add Xuanwo/xurl
```

2. Start your agent and ask the agent to summarize a thread:

```text
Please summarize this thread: agents://codex/xxx_thread
```


## Providers

| Provider | Query | Create |
| --- | --- | --- |
| <img src="https://ampcode.com/amp-mark-color.svg" alt="Amp logo" width="16" height="16" /> Amp | Yes | Yes |
| <img src="https://avatars.githubusercontent.com/u/14957082?s=24&v=4" alt="Codex logo" width="16" height="16" /> Codex | Yes | Yes |
| <img src="https://www.anthropic.com/favicon.ico" alt="Claude logo" width="16" height="16" /> Claude | Yes | Yes |
| <img src="https://www.google.com/favicon.ico" alt="Gemini logo" width="16" height="16" /> Gemini | Yes | Yes |
| <img src=".github/assets/pi-logo-dark.svg" alt="Pi logo" width="16" height="16" /> Pi | Yes | Yes |
| <img src="https://opencode.ai/favicon.ico" alt="OpenCode logo" width="16" height="16" /> OpenCode | Yes | Yes |

## Usage

Read an agent conversation:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Query provider threads:

```bash
xurl agents://codex
xurl 'agents://codex?q=spawn_agent'
xurl 'agents://claude?q=agent&limit=5'
```

Discover child targets:

```bash
xurl -I agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Drill down into a discovered child target:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

Start a new agent conversation:

```bash
xurl agents://codex -d "Draft a migration plan"
```

Continue an existing conversation:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 -d "Continue"
```

Create with query parameters:

```bash
xurl "agents://codex?workdir=%2FUsers%2Falice%2Frepo&add_dir=%2FUsers%2Falice%2Fshared&model=gpt-5" -d "Review this patch"
```

Save output:

```bash
xurl -o /tmp/conversation.md agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

## Command Reference

```bash
xurl [OPTIONS] <URI>
```

- `-I, --head`: output frontmatter/discovery info only.
- `-d, --data <DATA>`: write payload (repeatable).
  - text: `-d "hello"`
  - file: `-d @prompt.txt`
  - stdin: `-d @-`
- `-o, --output <PATH>`: write command output to file.
- `-I/--head` cannot be combined with `-d/--data`.

## URI Reference

Read/discovery URIs:

- `agents://<provider>[?q=<keyword>&limit=<n>]` (thread discovery/query mode)
- `agents://<provider>/<conversation_id>`
- `agents://<provider>/<conversation_id>/<child_id>`

Write URIs:

- `agents://<provider>?k=v` (create)
- `agents://<provider>/<conversation_id>` (append)

Read query keys:

- `q=<keyword>`: keyword search in provider thread data.
- `limit=<n>`: result count, default is `10`.

Create query keys:

- standard:
  - `workdir=<dir>`: working directory for the agent command (last-wins).
  - `add_dir=<dir>`: additional directory (repeatable).
- passthrough:
  - `k=v`: pass custom option with value.
  - `k` or `k=`: pass custom flag without value.
  - repeated keys preserve URI order.

Append query keys:

- all query keys are ignored with warnings (thread options are fixed at creation time).

Validation and ignore rules:

- reserved conflicting keys are ignored with `warning:` on stderr.
- empty `workdir` is rejected as an error.
- empty `add_dir` is ignored with warning.

Examples:

```text
agents://codex?q=spawn_agent&limit=10
agents://codex/threads/<conversation_id>
```

Legacy read compatibility:

- `<provider>://<conversation_id>`
- `<provider>://<conversation_id>/<child_id>`
