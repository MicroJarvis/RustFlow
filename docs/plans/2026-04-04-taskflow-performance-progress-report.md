# RustFlow Taskflow Performance Progress Report

Date: 2026-04-04

## Scope

This report summarizes the current performance work status after the executor/runtime mainline work, the algorithm-layer shrink pass, and the first stable round of pipeline restructuring.

It reflects the current stable worktree state, not only the last commit.

## Executive Summary

- The executor/runtime mainline is effectively closed for this round.
- The last committed baseline is `dd7f4ce` (`Optimize executor parking and root scheduling`).
- On top of that commit, the current stable worktree contains:
  - one small executor wake-observer cleanup
  - the Phase 3 algorithm-layer changes
  - the stable subset of Phase 4 pipeline restructuring
- `flow-core` and `flow-algorithms` test suites are currently green.
- `1.1 wait_for_all` is still open because the current toolchain in this workspace does not expose `AtomicUsize::wait/notify_*`.

## Plan Status

### Completed or effectively closed

- `1.2` Add executor continuation cache for ready graph tasks
- `1.3` Measure executor-only impact
- `2.1` Design a parent-linked runtime child completion path
- `2.2` Route `runtime_async` and `runtime_silent_async` through the lighter path
- `2.3` Measure recursion impact
- `3.1` Remove per-slot `Mutex<Option<T>>` output paths
- `3.2` Rework `scan` to direct-write per chunk
- `3.3` Add `sort` cutoffs and reduce merge overhead
- `4.1` Remove extra run coordination thread

### Partially completed

- `4.2` Tighten serial-stage bookkeeping
  - Stable part kept: executor-owned runtime scheduling, typed line slots, no extra coordinator thread
  - Experimental part not kept: token-ordered atomic serial-stage gate

### Still open

- `1.1` Reduce `wait_for_all` synchronization cost

## Stable Code State

### Executor and runtime

Stable baseline is the `dd7f4ce` line plus the current small cleanup in `flow-core/src/executor.rs`.

Included runtime/executor improvements:

- Per-worker two-phase parker (`prepare -> cancel|commit`)
- `thread::park/unpark`-based targeted wakeup
- `DriveMode::AllowPark` vs `DriveMode::NoPark`
- dedicated `RunCompletionEvent` for `wait_for_all`
- batched root scheduling and cached graph roots
- runtime join-scope and `corun_children` support
- small observer wake stabilization in `commit_park`

Primary reference:

- `docs/plans/2026-04-03-executor-worker-sleep-wake-refactor.md`

### Algorithm layer

Current stable algorithm work in `flow-algorithms` includes:

- internal `WriteOnceBuffer<T>` helper for lock-free single-assignment writeback
- `parallel_transform` and `parallel_reduce` switched away from per-slot `Mutex<Option<T>>`
- post-run spin waits removed after `executor.run(...).wait()`
- worker-aligned chunking helper for auto chunk selection
- sequential cutoffs for `transform`, `reduce`, `scan`, and `sort`
- `scan` changed from chunk `Vec` flattening to chunk-local output plus one final append pass
- `sort` merge rounds avoid unnecessary async layers when chunk count is already small

### Pipeline layer

Current stable pipeline work in `flow-core/src/pipeline.rs` and `flow-core/src/runtime.rs` includes:

- removed the old extra coordinator thread (`spawn_run_handle`)
- pipeline execution is now scheduled through executor-owned runtime runners
- `DataPipeline` now uses typed per-line storage instead of type-erased boxed payload handoff
- added `RuntimeCtx::silent_result_async`
- pipeline lines are scheduled as silent runtime children and collected with `corun_children`

This is the stable Phase 4 baseline currently worth keeping.

## Verification Status

Current stable verification status:

- `cargo test -p flow-core -- --nocapture`: PASS
- `cargo test -p flow-core --test pipelines -- --nocapture`: PASS
- `cargo test -p flow-core --test scalable_data_pipelines -- --nocapture`: PASS
- `cargo test -p flow-algorithms -- --nocapture`: PASS

Observer stability note:

- `flow-core/tests/observers.rs` was adjusted to make wake observation deterministic by forcing a second run after workers have had time to sleep.

## Benchmark Snapshot

### Phase 2 executor/runtime

Reference summary:

- `docs/plans/2026-04-03-executor-worker-sleep-wake-refactor.md`

Main takeaway:

- recursive runtime-heavy cases improved materially
- `fibonacci`, `integrate`, and `nqueens` all moved in the right direction
- `thread_pool` narrowed but was still somewhat noise-sensitive

### Phase 3 algorithms

Primary stable report:

- `benchmarks/reports/taskflow_compare/phase3_algorithms_r3_current/taskflow_vs_rustflow_report.md`

Key readout:

- `reduce_sum` small and medium fixed-cost points improved clearly
- `scan` improved versus the first naive direct-write attempt, but the large tail is still red
- `sort` is improved for small sizes but remains meaningfully behind on larger inputs
- `primes` remains a major red item and is not addressed by the current algorithm changes

Important ratios from the current `r3` report:

- `reduce_sum`
  - `100000`: `0.721x`
  - `1000000`: `1.838x`
- `scan`
  - `10000`: `1.846x`
  - `100000`: `5.674x`
- `sort`
  - `10000`: `2.011x`
  - `100000`: `3.191x`
- `primes`
  - `10000`: `11.062x`
  - `100000`: `6.954x`

### Phase 4 pipelines

Primary stable report:

- `benchmarks/reports/taskflow_compare/phase4_pipelines_r3_current_v2/taskflow_vs_rustflow_report.md`

Key readout:

- `data_pipeline` is improved structurally but still about `2x` to `2.8x` at the larger sizes
- `graph_pipeline` narrowed into the `1.5x` to `1.9x` band on the main points
- `linear_pipeline` is still around `1.7x` to `2.0x` on most larger points
- `wavefront` remains clearly red and is not a Phase 4 core acceptance item

Important ratios from the current stable `r3` pipeline report:

- `data_pipeline`
  - `64`: `1.991x`
  - `1024`: `2.754x`
- `graph_pipeline`
  - `911`: `1.471x`
  - `12904`: `1.724x`
- `linear_pipeline`
  - `64`: `1.993x`
  - `1024`: `1.956x`

## Experiments Run But Not Kept

### Wait-assist in external `RunHandle::wait`

This experiment tried to let external waiters assist executor progress.

Outcome:

- sometimes improved `thread_pool`
- sometimes regressed mixed-case `r3`
- too noisy to keep in the stable baseline

Status:

- reverted

Related reports:

- `benchmarks/reports/taskflow_compare/phase2_wait_assist_probe/taskflow_vs_rustflow_report.md`
- `benchmarks/reports/taskflow_compare/phase2_wait_assist_runtime_probe/taskflow_vs_rustflow_report.md`

### Token-ordered atomic serial-stage gate for pipelines

This experiment replaced post-stage serial `Mutex` guarding with token-ordered atomic gating.

Outcome:

- initially introduced a live-lock risk because the serial-stage wait path recursively drove more line tasks
- after fixing the live-lock with non-recursive waiting, results were mixed
- `graph_pipeline` moved closer in some points
- `linear_pipeline` regressed materially
- not stable enough to keep

Status:

- reverted from the stable pipeline baseline

Related probe:

- `benchmarks/reports/taskflow_compare/phase4_data_pipeline_probe_v3c/taskflow_vs_rustflow_report.md`

### `wait_for_all` atomic wait/notify

Planned `1.1` direction:

- replace `RunCompletionEvent` with direct atomic wait/notify on `active_runs`

Outcome:

- blocked by toolchain support in this workspace
- current Rust here does not expose the required `AtomicUsize::wait/notify_*` surface

Status:

- reverted
- task remains open

## Current Risks and Constraints

- The worktree is intentionally dirty and includes benchmark outputs plus several user-side experimental files.
- `benchmarks/src/bin/taskflow_compare.rs` is modified in the worktree and should be treated carefully.
- `1.1` cannot be finished with the originally planned implementation unless the toolchain is upgraded or a different low-overhead design is chosen.
- `4.2` likely needs a more Taskflow-like scheduler structure, not another round of local lock replacement.

## Recommended Next Steps

### 1. Freeze the current stable baseline

Recommended baseline to keep:

- executor/runtime changes through `dd7f4ce`
- current stable algorithm-layer changes
- stable pipeline restructuring without the serial-stage atomic gating experiment

### 2. Treat pipeline follow-up as a scheduler rewrite task, not a lock micro-tuning task

Best next Phase 4 direction:

- move toward a Taskflow-style pipeline scheduler with explicit per-line/per-stage state and join counters
- avoid further local experiments that only swap `Mutex` for atomics inside the current line-loop model

### 3. Leave `1.1` open until the toolchain question is settled

Options:

- upgrade the toolchain and retry atomic wait/notify
- or design a lower-overhead completion event that still works on the current stable toolchain

### 4. If choosing between Phase 3 and Phase 4 next

Recommendation:

- prioritize Phase 4 scheduler structure first
- revisit Phase 3 only if you want to specifically attack `sort` with a more Taskflow-like sorting strategy
- `primes` should be treated as a separate algorithmic gap, not as fallout from executor work

## Current Reference Files

- Executor/runtime summary:
  - `docs/plans/2026-04-03-executor-worker-sleep-wake-refactor.md`
- Main execution plan:
  - `doc/taskflow-performance-execution-plan.md`
- Stable algorithm report:
  - `benchmarks/reports/taskflow_compare/phase3_algorithms_r3_current/taskflow_vs_rustflow_report.md`
- Stable pipeline report:
  - `benchmarks/reports/taskflow_compare/phase4_pipelines_r3_current_v2/taskflow_vs_rustflow_report.md`
