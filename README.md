# xURL

`xURL` is a client for AI agent URLs.

> Also known as **Xuanwo's URL**.

## What xURL Can Do

- Read an agent conversation as markdown.
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

## Usage

Read an agent conversation:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Discover child targets:

```bash
xurl -I agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

Drill down into a discovered child target:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
```

OpenCode child linkage is validated via sqlite `session.parent_id`.
Start a new agent conversation:

```bash
xurl agents://codex -d "Draft a migration plan"
```

Continue an existing conversation:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 -d "Continue"
```

Save output:

```bash
xurl -o /tmp/conversation.md agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
```

## Command Reference

```bash
xurl [OPTIONS] <URI>
```

Options:

- `-I, --head`: output frontmatter/discovery info only.
- `-d, --data <DATA>`: write payload (repeatable).
- `-o, --output <PATH>`: write command output to file.

`--data` supports:

- text: `-d "hello"`
- file: `-d @prompt.txt`
- stdin: `-d @-`

## Providers

| Provider | Query | Create |
| --- | --- | --- |
| <img src="https://ampcode.com/amp-mark-color.svg" alt="Amp logo" width="16" height="16" /> Amp | Yes | Yes |
| <img src="https://avatars.githubusercontent.com/u/14957082?s=24&v=4" alt="Codex logo" width="16" height="16" /> Codex | Yes | Yes |
| <img src="https://www.anthropic.com/favicon.ico" alt="Claude logo" width="16" height="16" /> Claude | Yes | Yes |
| <img src="https://www.google.com/favicon.ico" alt="Gemini logo" width="16" height="16" /> Gemini | Yes | Yes |
| <img src=".github/assets/pi-logo-dark.svg" alt="Pi logo" width="16" height="16" /> Pi | Yes | Yes |
| <img src="https://opencode.ai/favicon.ico" alt="OpenCode logo" width="16" height="16" /> OpenCode | Yes | Yes |

## URI Formats

```text
agents://<provider>/<conversation_target>
```

For examples:

```text
agents://codex/threads/<conversation_id>
```
