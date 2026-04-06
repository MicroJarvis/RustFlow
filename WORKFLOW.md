---
tracker:
  kind: linear
  project_slug: "REPLACE_WITH_LINEAR_PROJECT_SLUG"
  active_states:
    - Todo
    - In Progress
    - Human Review
    - Merging
    - Rework
  terminal_states:
    - Closed
    - Cancelled
    - Canceled
    - Duplicate
    - Done
  polling:
    interval_ms: 5000
workspace:
  root: ~/code/symphony-workspaces/rustflow
  hooks:
    after_create: |
      git clone --depth 1 git@github.com:MicroJarvis/RustFlow.git .
      if command -v cargo >/dev/null 2>&1; then
        cargo fetch
      fi
agent:
  max_concurrent_agents: 4
  max_turns: 20
codex:
  command: codex --config shell_environment_policy.inherit=all --config model_reasoning_effort=high --model gpt-5.3-codex app-server
  approval_policy: never
  thread_sandbox: workspace-write
  turn_sandbox_policy:
    type: workspaceWrite
---

You are working on a RustFlow Linear ticket `{{ issue.identifier }}`.

{% if attempt %}
Continuation context:
- This is retry attempt #{{ attempt }} because the ticket is still in an active state.
- Resume from the current workspace state instead of restarting from scratch.
- Do not repeat already-completed investigation or validation unless needed for new code changes.
- Do not end the turn while the ticket remains in an active state unless you are blocked by missing required permissions, auth, or secrets.
{% endif %}

Issue context:

Identifier: {{ issue.identifier }}
Title: {{ issue.title }}
Current status: {{ issue.state }}
Labels: {{ issue.labels }}
URL: {{ issue.url }}
Description:
{% if issue.description %}
{{ issue.description }}
{% else %}
No description provided.
{% endif %}

Instructions:

1. This is an unattended orchestration session. Never ask a human to perform follow-up actions.
2. Only stop early for a true blocker such as missing required auth, permissions, secrets, or external services.
3. Final message must report completed actions and blockers only. Do not include "next steps for user".
4. Work only inside the provided repository copy.

## Prerequisite: Linear MCP or `linear_graphql` tool is available

The agent should be able to talk to Linear, either via a configured Linear MCP server or Symphony's injected `linear_graphql` tool. If neither is present, stop and report that Linear is not configured for the session.

## RustFlow repository context

- Rust workspace root with crates:
  - `flow-core`
  - `flow-algorithms`
  - `benchmarks`
- `taskflow/` is a checked-in upstream C++ reference tree used for comparison and benchmarking. Do not modify it unless the ticket explicitly calls for benchmark-driver parity or Taskflow fixture work.
- Primary validation commands:
  - focused crate tests when scope is narrow
  - `cargo test --workspace` for broad or cross-cutting changes
  - benchmark commands only when the ticket explicitly calls for performance measurement
- Performance tickets should record exact benchmark commands and before/after numbers in the workpad before moving to review.
- Prefer small, reviewable diffs. Preserve public API stability unless the ticket explicitly changes it.

## Related skills

- `linear`: interact with Linear through `linear_graphql`.
- `commit`: produce clean, logical commits during implementation.
- `push`: publish the current branch and create or update a PR.
- `pull`: merge `origin/master` into the current branch when sync is needed.
- `land`: when the ticket reaches `Merging`, follow the land flow.

## Status map

- `Backlog` -> out of scope for this workflow; do not modify.
- `Todo` -> queued; immediately transition to `In Progress` before active work.
- `In Progress` -> implementation actively underway.
- `Human Review` -> PR is attached and validated; waiting on human approval.
- `Merging` -> approved by human; execute the `land` skill flow.
- `Rework` -> reviewer requested changes; planning and implementation required.
- `Done` -> terminal state; no further action required.

## Step 0: Determine current ticket state and route

1. Fetch the issue by explicit ticket ID.
2. Read the current state.
3. Route to the matching flow.
4. If a branch PR exists and is `CLOSED` or `MERGED`, create a fresh branch from `origin/master` and restart execution flow as a new attempt.
5. For `Todo` tickets, do startup sequencing in this exact order:
   - move the issue to `In Progress`
   - find or create the `## Codex Workpad` bootstrap comment
   - only then begin analysis, planning, and implementation work

## Step 1: Start or continue execution

1. Find or create a single persistent workpad comment for the issue using the marker header `## Codex Workpad`.
2. Reconcile that workpad before making new edits:
   - check off already-finished items
   - refresh the plan for current scope
   - ensure acceptance criteria and validation steps are current
3. Add a compact environment stamp at the top:
   - format: `host:/absolute/workspace/path@commit`
4. Keep all progress, validation notes, and handoff details in that one workpad comment.
5. Reproduce first when applicable. Make the target behavior explicit before changing code.
6. Update issue state only when the quality bar for that state is actually met.

## Execution rules

- Prefer focused tests first, then broader verification when the diff crosses crate boundaries.
- If the ticket changes public behavior, docs, or benchmark expectations, update the relevant docs in the repo.
- Do not leave generated reports or benchmark artifacts unstated; either keep them intentionally or omit them intentionally.
- Use `origin/master` as the default base branch for sync and PR work in this repository.

## Completion bar

Before moving a ticket to `Human Review`, ensure:

- code changes are complete for scope
- validation was run and recorded in the workpad
- PR exists and points at the current branch
- any benchmark-sensitive claims include exact commands and measured results
