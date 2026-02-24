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
xurl -I agents://amp/T-019c0797-c402-7389-bd80-d785c98df295
xurl -I agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592
xurl -I agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
xurl agents://gemini/29d207db-ca7e-40ba-87f7-e14c9de60613
xurl -I agents://gemini/29d207db-ca7e-40ba-87f7-e14c9de60613
xurl agents://gemini/29d207db-ca7e-40ba-87f7-e14c9de60613/2b112c8a-d80a-4cff-9c8a-6f3e6fbaf7fb
xurl -I agents://opencode/ses_43a90e3adffejRgrTdlJa48CtE
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4
xurl -I agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f
```

Drill down into a discovered child target:

```bash
xurl agents://amp/T-019c0797-c402-7389-bd80-d785c98df295/T-1abc0797-c402-7389-bd80-d785c98df295
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495
xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/72b3a4a8-4f08-40af-8d7f-8b2c77584e89
xurl agents://pi/12cb4c19-2774-4de4-a0d0-9fa32fbae29f/d1b2c3d4
xurl agents://opencode/ses_43a90e3adffejRgrTdlJa48CtE/ses_3k2j1h9g8f7d
```

OpenCode child linkage is validated via sqlite `session.parent_id`.
Start a new agent conversation:

```bash
xurl agents://codex -d "Draft a migration plan"
xurl agents://claude -d @prompt.txt
```

Continue an existing conversation:

```bash
xurl agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592 -d "Continue"
cat prompt.md | xurl agents://claude/2823d1df-720a-4c31-ac55-ae8ba726721f -d @-
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
| <img src="https://ampcode.com/amp-mark-color.svg" alt="Amp logo" width="16" height="16" /> Amp | Yes | No |
| <img src="https://avatars.githubusercontent.com/u/14957082?s=24&v=4" alt="Codex logo" width="16" height="16" /> Codex | Yes | Yes |
| <img src="https://www.anthropic.com/favicon.ico" alt="Claude logo" width="16" height="16" /> Claude | Yes | Yes |
| <img src="https://www.google.com/favicon.ico" alt="Gemini logo" width="16" height="16" /> Gemini | Yes | No |
| <img src=".github/assets/pi-logo-dark.svg" alt="Pi logo" width="16" height="16" /> Pi | Yes | No |
| <img src="https://opencode.ai/favicon.ico" alt="OpenCode logo" width="16" height="16" /> OpenCode | Yes | No |

## URI Formats

```text
agents://<provider>/<conversation_target>
```

For examples:

```text
agents://amp/<thread_id>
agents://amp/<main_thread_id>/<child_thread_id>
agents://codex/<conversation_id>
agents://codex/<main_conversation_id>/<agent_id>
agents://claude/<conversation_id>
agents://claude/<main_conversation_id>/<agent_id>
agents://pi/<conversation_id>/<child_session_id>
agents://pi/<conversation_id>/<entry_id>
```
