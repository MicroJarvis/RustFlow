# Taskflow Performance Execution Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce the largest RustFlow vs Taskflow gaps without regressing existing correctness guarantees or current winning benchmarks.

**Architecture:** Keep the public API stable and optimize from the inside out. Start with executor hot paths that affect many benchmarks at once, then tighten runtime recursion, then shrink algorithm-layer overhead, and finally remove structural costs in pipeline execution.

**Tech Stack:** Rust stable, `flow-core`, `flow-algorithms`, workspace tests, `benchmarks` taskflow comparison harness.

---

## Context

This plan is the execution companion to:

- `doc/taskflow-performance-plan.md`
- `benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_report.md`
- `benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_analysis.md`

The current priority order remains:

1. Executor tiny-task fixed cost
2. Runtime recursion cost on `fibonacci`
3. Algorithm-layer overhead in `scan` / `sort` / `reduce`-family paths
4. Pipeline structural overhead

## Guardrails

- Preserve current API shape unless a change is strongly justified.
- Preserve cancellation, panic propagation, and observer semantics.
- Preserve current strong results on `integrate`, `skynet`, and `matrix_multiplication`.
- Prefer improvements that help multiple benchmarks over benchmark-specific tuning.
- After each phase, run targeted tests first and only then run targeted benchmarks.

## Phase 1: Executor Hot Path

**Target benchmarks:** `async_task`, `linear_chain`, `binary_tree`, `embarrassing_parallelism`, `thread_pool`, secondary `graph_traversal`

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/asyncs.rs`
- Verify: `flow-core/tests/basics.rs`
- Verify: `flow-core/tests/runtimes.rs`
- Verify: `flow-core/tests/observers.rs`

**Design:**
- Replace heavyweight executor-wide completion waiting with atomic wait/notify on `active_runs`.
- Add a ready-task continuation path so one freshly readied graph task can execute on the current worker without a queue round-trip.
- Reuse the same continuation path inside worker-inline wait loops (`corun_handle`, `corun_handles`) to reduce queue churn during nested execution.
- Keep queue semantics and notifier behavior otherwise intact for the first pass; do not combine this phase with a full queue rewrite.

**Implementation tasks:**

### Task 1.1: Reduce `wait_for_all` synchronization cost

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/basics.rs`
- Verify: `flow-core/tests/asyncs.rs`

**Steps:**
1. Replace `active_runs_ready` + `active_runs_wait` with atomic wait/notify on `active_runs`.
2. Keep the existing correctness contract of `Executor::wait_for_all`.
3. Run:
   - `cargo test -p flow-core basics::wait_for_all_blocks_until_all_runs_finish -- --exact`
   - `cargo test -p flow-core asyncs::silent_async_is_tracked_by_wait_for_all -- --exact`

### Task 1.2: Add executor continuation cache for ready graph tasks

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/basics.rs`
- Verify: `flow-core/tests/runtimes.rs`
- Verify: `flow-core/tests/observers.rs`

**Steps:**
1. Introduce a worker-local cached continuation slot in the worker loop.
2. Return one ready graph successor from `execute_graph` for immediate execution on the same worker instead of always enqueueing it.
3. Preserve `outstanding` accounting when a continuation is cached instead of enqueued.
4. Mirror the same behavior inside `wait_handles_inline`.
5. Run:
   - `cargo test -p flow-core basics::run_n_executes_n_times -- --exact`
   - `cargo test -p flow-core runtimes::runtime_async_can_wait_inline_on_a_single_worker -- --exact`
   - `cargo test -p flow-core observers::observer_receives_task_and_worker_lifecycle_events -- --exact`

### Task 1.3: Measure executor-only impact

**Files:**
- No code changes required
- Verify: `benchmarks/src/bin/taskflow_compare.rs`

**Steps:**
1. Run:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 1 --cases async_task,linear_chain,binary_tree,thread_pool --output-dir benchmarks/reports/taskflow_compare/phase1`
2. Compare ratios against the current baseline report.
3. Delete temporary benchmark output if it is not intended to become a committed baseline.

## Phase 2: Runtime Recursion Fast Path

**Target benchmarks:** `fibonacci`, secondary `nqueens`

**Files:**
- Modify: `flow-core/src/executor.rs`
- Modify: `flow-core/src/runtime.rs`
- Modify: `flow-core/src/async_handle.rs`
- Verify: `flow-core/tests/runtimes.rs`
- Verify: `benchmarks/src/bin/taskflow_compare.rs`

**Design:**
- Introduce a lighter-weight runtime child path that keeps parent/child synchronization closer to executor state instead of always allocating the full `RunHandle + AsyncState` stack.
- Keep the existing public `runtime_async` API, but make runtime-spawned children cheaper internally.
- Preserve worker-inline waiting and panic propagation.

**Implementation tasks:**

### Task 2.1: Design a parent-linked runtime child completion path

**Files:**
- Modify: `flow-core/src/executor.rs`
- Modify: `flow-core/src/runtime.rs`

**Steps:**
1. Introduce internal executor state for runtime-owned child tasks.
2. Keep ownership and lifetime explicit; do not rely on stack references across worker boundaries.
3. Preserve cancellation checks on the parent runtime.

### Task 2.2: Route `runtime_async` and `runtime_silent_async` through the lighter path

**Files:**
- Modify: `flow-core/src/executor.rs`
- Modify: `flow-core/src/async_handle.rs`
- Verify: `flow-core/tests/runtimes.rs`

**Steps:**
1. Keep the public return types stable.
2. Ensure `wait_async`, `corun_handle`, and `corun_handles` still surface errors and panics correctly.
3. Run:
   - `cargo test -p flow-core runtimes -- --nocapture`

### Task 2.3: Measure recursion impact

**Steps:**
1. Run:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 1 --cases fibonacci,nqueens,integrate --output-dir benchmarks/reports/taskflow_compare/phase2`
2. Confirm `integrate` does not regress while `fibonacci` improves.

## Phase 3: Algorithm-Layer Shrink

**Target benchmarks:** `scan`, `sort`, `reduce_sum`, `primes`, secondary `for_each`, `black_scholes`

**Files:**
- Modify: `flow-algorithms/src/reduce_transform.rs`
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Modify: `flow-algorithms/src/parallel_for.rs`
- Verify: `flow-algorithms` tests if present
- Verify: `benchmarks/src/bin/taskflow_compare.rs`

**Design:**
- Remove per-output locking and post-run spin waits.
- Write directly into preallocated output buffers where possible.
- Add explicit small-input cutoffs to avoid over-parallelization.
- Keep chunking aligned with executor worker count.

**Implementation tasks:**

### Task 3.1: Remove per-slot `Mutex<Option<T>>` output paths

**Files:**
- Modify: `flow-algorithms/src/reduce_transform.rs`

**Steps:**
1. Introduce internal preallocated writeback helpers.
2. Remove `remaining` spin-wait loops after `executor.run(...).wait()`.
3. Validate with focused algorithm tests or benchmark smoke checks.

### Task 3.2: Rework `scan` to direct-write per chunk

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`

**Steps:**
1. Keep block-prefix logic but avoid chunk `Vec` flattening.
2. Prefer writing into a single output allocation.
3. Benchmark `scan` before and after.

### Task 3.3: Add `sort` cutoffs and reduce merge overhead

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`

**Steps:**
1. Add a sequential cutoff for small inputs.
2. Reduce extra async layers during merge rounds.
3. Benchmark `sort` before and after.

## Phase 4: Pipeline Structural Cleanup

**Target benchmarks:** `data_pipeline`, `graph_pipeline`, secondary `linear_pipeline`, `wavefront`

**Files:**
- Modify: `flow-core/src/pipeline.rs`
- Verify: `flow-core/tests/pipelines.rs`
- Verify: `flow-core/tests/scalable_data_pipelines.rs`

**Design:**
- Remove the extra coordinator thread created by `spawn_run_handle`.
- Keep pipeline execution on executor workers as much as possible.
- Preserve typed pipeline ergonomics and current correctness behavior.

**Implementation tasks:**

### Task 4.1: Remove extra run coordination thread

**Files:**
- Modify: `flow-core/src/pipeline.rs`

**Steps:**
1. Replace thread-spawned coordination with executor-owned scheduling.
2. Preserve cancellation propagation and error aggregation.
3. Run:
   - `cargo test -p flow-core pipelines scalable_data_pipelines -- --nocapture`

### Task 4.2: Tighten serial-stage bookkeeping

**Files:**
- Modify: `flow-core/src/pipeline.rs`

**Steps:**
1. Reduce lock scope around token assignment and serial-stage transitions.
2. Re-evaluate whether `DataPipeline` needs any hot-path type-erased boxing changes after coordination-thread removal.

## Benchmark Cadence

After each phase:

1. Run the smallest relevant unit/integration tests first.
2. Run the smallest relevant benchmark subset.
3. Record:
   - improved cases
   - unchanged cases
   - regressed cases
4. Only then move to the next phase.

## Progress Notes

- 2026-04-01:
  - Implemented a worker-local continuation path in `flow-core/src/executor.rs`.
  - The continuation path is used both in the worker loop and in inline wait loops used by runtime corun-style execution.
  - Attempted to replace `wait_for_all` synchronization with atomic wait/notify, but the current toolchain in this workspace does not expose `AtomicUsize::wait/notify`; this item remains open and needs either a toolchain upgrade or a different low-overhead design.
  - Tried switching single-task enqueue wakeups from broadcast to single-worker wakeup; reverted after targeted benchmarks showed a clear regression on `thread_pool`.
  - Added an internal runtime spawn context so `runtime.executor().runtime_async(...)` and `runtime.executor().runtime_silent_async(...)` can inherit the current worker hint and parent cancellation view without changing the public API.
  - Added a dedicated `RuntimeAsyncState<T>` fast path for runtime-spawned value tasks, so recursive `runtime_async` calls no longer pay the full `RunHandle + AsyncState` stack on the hot path.
  - Kept `runtime_silent_async` on the stable `RunHandle` surface for now, but routed runtime-originated submissions through the current worker and parent cancellation path.
  - Added a focused runtime test to confirm a runtime child still observes parent-run cancellation.
- 2026-04-01 phase-1 benchmark subset (`async_task`, `binary_tree`, `linear_chain`, `thread_pool`, 4 threads, 1 round):
  - `linear_chain` improved substantially at the large tail; for example, `65536` dropped from roughly `7.2x` Taskflow to roughly `2.4x`.
  - `thread_pool` improved materially but is still behind; for example, `100` iterations dropped from roughly `5.0x` to roughly `3.1x`.
  - `async_task` improved at larger sizes but remains clearly red; for example, `65536` dropped from roughly `4.3x` to roughly `3.2x`.
  - `binary_tree` improved in several points but remains inconsistent and still needs more executor-path work.
- 2026-04-01 phase-2 benchmark subset (`fibonacci`, `nqueens`, `integrate`, 4 threads, 1 round):
  - `fibonacci` improved sharply at the large tail; for example, `fib(20)` dropped from roughly `45.6x` Taskflow to roughly `9.3x`, and `fib(18)` dropped from roughly `44.1x` to roughly `21.5x`.
  - `nqueens` also improved at the red tail; for example, `10` queens dropped from roughly `2.0x` Taskflow to roughly `1.3x`.
  - `integrate` stayed strongly green in this subset; for example, size `500` remained comfortably faster at roughly `0.05x` Taskflow.
  - The remaining `fibonacci` gap is now smaller but still large enough that Phase 2 is not done; the next likely gain is slimming `runtime_silent_async`/run-handle completion on the recursive hot path even further.

## Initial Execution Order

1. Phase 1 / Task 1.1
2. Phase 1 / Task 1.2
3. Phase 1 / Task 1.3
4. Phase 2 / Task 2.1
5. Phase 2 / Task 2.2
6. Phase 2 / Task 2.3
7. Phase 3 / Task 3.1
8. Phase 3 / Task 3.2
9. Phase 3 / Task 3.3
10. Phase 4 / Task 4.1
11. Phase 4 / Task 4.2

## Current Execution Status

- [ ] Phase 1 / Task 1.1
- [x] Phase 1 / Task 1.2
- [x] Phase 1 / Task 1.3
- [x] Phase 2 / Task 2.1
- [ ] Phase 2 / Task 2.2
- [ ] Phase 2 / Task 2.3
- [ ] Phase 3 / Task 3.1
- [ ] Phase 3 / Task 3.2
- [ ] Phase 3 / Task 3.3
- [ ] Phase 4 / Task 4.1
- [ ] Phase 4 / Task 4.2
