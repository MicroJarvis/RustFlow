# Symphony Setup For RustFlow

This repository now includes a repo-local Symphony workflow file at [`WORKFLOW.md`](../WORKFLOW.md) and supporting repo skills under [`.codex/skills`](../.codex/skills).

## What Was Added

- `WORKFLOW.md`
  - RustFlow-specific Symphony workflow prompt and orchestration config
- `.codex/skills/commit/SKILL.md`
- `.codex/skills/pull/SKILL.md`
- `.codex/skills/push/SKILL.md`
- `.codex/skills/land/SKILL.md`
- `.codex/skills/linear/SKILL.md`

## Before First Run

1. Install Symphony using the official README:
   - https://github.com/openai/symphony/blob/main/elixir/README.md
2. Export your Linear personal API key:

```bash
export LINEAR_API_KEY=your_linear_token
```

3. Edit [`WORKFLOW.md`](../WORKFLOW.md) and replace:
   - `REPLACE_WITH_LINEAR_PROJECT_SLUG`

4. Make sure your Linear team workflow contains the states expected by the workflow:
   - `Todo`
   - `In Progress`
   - `Human Review`
   - `Merging`
   - `Rework`
   - terminal states such as `Done`, `Closed`, `Cancelled`, and `Duplicate`

5. Optionally change `workspace.root` if you do not want Symphony workspaces under `~/code/symphony-workspaces/rustflow`.

## How This Workflow Is Customized For RustFlow

- clones `git@github.com:MicroJarvis/RustFlow.git`
- treats `origin/master` as the default base branch
- assumes Rust-focused validation such as:
  - focused `cargo test -p ...`
  - `cargo test --workspace`
  - benchmark commands only for performance tickets
- treats `taskflow/` as a checked-in reference tree that should not be modified unless the issue explicitly requires it
- lowers `max_concurrent_agents` to `4` to avoid excessive Rust build contention

## Start Symphony

From the Symphony checkout:

```bash
cd symphony/elixir
mise trust
mise install
mise exec -- mix setup
mise exec -- mix build
mise exec -- ./bin/symphony /Users/tfjiang/Projects/RustFlow/WORKFLOW.md
```

If you do not use `mise`, follow the official README for equivalent Elixir runtime setup first.

## Notes

- The workflow file is valid YAML front matter plus a Markdown prompt body, which is what Symphony expects.
- The `linear` skill assumes Symphony injects the `linear_graphql` tool during app-server sessions.
- The repo-local `push` and `land` skills were adapted for RustFlow and do not depend on Symphony's Elixir-specific `mix` tooling.
