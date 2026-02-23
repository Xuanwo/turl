# Subagent URI Design Across Providers

## Status

Proposed

## Context

`turl` currently resolves a single thread URI into one local thread file and renders a timeline view. This works for primary conversations, but it does not provide a first-class way to inspect subagent lifecycle state or drill down into a specific subagent context under a parent thread.

The existing URI behavior is inconsistent with subagent use cases because it only models one `session_id` and does not encode parent/child scope in the URI itself.

## Goals

- Keep backward compatibility for existing provider URIs.
- Use one URI shape across providers for subagent drill-down.
- Support both aggregate status and single-agent drill-down.
- Use one explicit CLI mode switch for aggregate listing.
- Make raw output stable for automation consumers.

## Non-Goals

- Defining provider-specific transport details for remote RPC.
- Replacing the existing single-thread render pipeline.
- Introducing provider-specific query parameter syntax for subagent views.

## Unified URI Model

### Existing URIs (unchanged)

- `codex://<thread_id>`
- `codex://threads/<thread_id>`
- `claude://<session_id>`
- Other providers continue their current single-thread form.

### New Drill-Down URI (provider-consistent)

- Drill down into one subagent:
  - `<provider>://<main_thread_id>/<agent_id>`

## CLI Mode Model

### Aggregate Listing

- Aggregate subagents under a parent thread is triggered by `--list`:
  - `turl '<provider>://<main_thread_id>' --list`

### Single-Agent Drill-Down

- Drill-down view is path-based:
  - `turl '<provider>://<main_thread_id>/<agent_id>'`

### Mode Constraints

- `--list` requires a parent-thread URI (`<provider>://<main_thread_id>`).
- `--list` is invalid with drill-down URI (`<provider>://<main_thread_id>/<agent_id>`).
- `--list` can be combined with `--raw` for machine-readable aggregate output.

## Provider Mapping

### Codex

- `agent_id` is treated as a subagent identifier scoped by `main_thread_id`.
- Current local evidence shows `agent_id` commonly equals child thread id, but implementation must still validate parent-child relation.
- Parent lifecycle is inferred from tool calls such as `spawn_agent`, `wait`, `send_input`, `resume_agent`, and `close_agent`.

### Claude

- `agent_id` maps to transcript field `agentId`.
- Candidate files are discovered from:
  - `<project>/<main_session_id>/subagents/agent-*.jsonl`
  - `<project>/agent-*.jsonl` filtered by `sessionId == main_session_id`
- Validation should require `isSidechain == true` and matching `sessionId`.

## Resolution Flow

### Aggregate: `<provider>://<main> --list`

1. Resolve and load parent thread.
2. Discover child/subagent records for that provider.
3. Validate parent-child linkage.
4. Build per-agent status summary.
5. Render aggregate markdown or emit structured raw JSON.

### Drill-Down: `<provider>://<main>/<agent>`

1. Resolve and load parent thread.
2. Locate target agent/thread using provider mapping rules.
3. Validate linkage between parent and agent.
4. Build lifecycle summary from parent and excerpt from agent transcript.
5. Render combined view or emit structured raw JSON.

## Status Normalization

Preferred normalized states:

- `pendingInit`
- `running`
- `completed`
- `errored`
- `shutdown`
- `notFound`

Each response should include `status_source`, for example:

- `protocol`
- `parent_rollout`
- `child_rollout`
- `inferred`

## Output Contract

### Markdown

Use a consistent section layout:

1. `Agent Status Summary`
2. `Lifecycle (Parent Thread)`
3. `Thread Excerpt (Child Thread)`

### Raw

Return stable machine-readable fields:

- `query`
- `relation`
- `lifecycle`
- `status`
- `status_source`
- `child_thread`
- `excerpt`
- `warnings`

## Compatibility Rules

- Existing single-thread URIs must behave exactly as today.
- New subagent support must not require query parameters.
- Parser must reject malformed path shapes with actionable errors.
- CLI must reject invalid mode combinations with actionable errors.

## Risks

- Codex local rollout may miss complete collaboration events.
- Claude status is inferred from local transcripts, not protocol-native.
- `agent_id == child_thread_id` in Codex is observational, not guaranteed by contract.

## Test Scope

- URI parsing unit tests:
  - existing URIs
  - `<provider>://<main>/<agent>`
  - malformed path rejection
- CLI argument tests:
  - `<provider>://<main> --list`
  - `<provider>://<main>/<agent>` without `--list`
  - invalid `--list` with `<provider>://<main>/<agent>`
- Provider tests:
  - Codex parent-child validation and lifecycle extraction
  - Claude file discovery in both known layouts
- CLI integration tests:
  - markdown and `--raw` for aggregate and drill-down URIs
  - stderr warnings and exit-code behavior unchanged
