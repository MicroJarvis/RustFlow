# Post-Lockfree Queue Performance Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the remaining hot-path synchronization and structure costs after the lock-free queue migration, and close the largest RustFlow vs Taskflow gaps without regressing current winners.

**Architecture:** Keep the `crossbeam-deque` queue layer. Focus next on the parts still paying lock or structural overhead around that queue: completion waiting, runtime child submission, flow build/snapshot cost, algorithm adapter parity, and pipeline structure. Prefer internal changes over public API changes unless the internal path cannot close the gap.

**Tech Stack:** Rust stable, `flow-core`, `flow-algorithms`, `benchmarks`, workspace tests, `taskflow_compare` harness, local Taskflow checkout under `taskflow/`.

---

## Context

This plan is based on:

- `flow-core/src/executor.rs`
- `flow-core/src/runtime.rs`
- `flow-core/src/flow.rs`
- `flow-core/src/task_group.rs`
- `flow-algorithms/src/find_scan_sort.rs`
- `flow-algorithms/src/reduce_transform.rs`
- `flow-core/src/pipeline.rs`
- `benchmarks/reports/taskflow_compare/current_live_20260405/taskflow_vs_rustflow_report.md`

Current highest-value observations:

1. The executor queue layer is already lock-free.
2. The hottest remaining locks are around `RunHandle` / `wait_for_all`, not the queue.
3. Runtime child submission still carries more bookkeeping than Taskflow's `TaskGroup`-style path.
4. `Flow` graph mutation and `snapshot()` still pay `RwLock` and clone costs on every build/run.
5. Several red benchmarks are still partly benchmark-shape or adapter-shape issues, not only core scheduler issues.

## Guardrails

- Keep current public API stable unless a change is strongly justified.
- Preserve cancellation, panic propagation, observer behavior, and semaphore semantics.
- Do not regress current wins on `integrate`, `skynet`, `reduce_sum`, and `graph_traversal`.
- Separate "core runtime bottleneck" work from "benchmark adapter parity" work in both commits and reports.
- After each task, run the smallest relevant test set first, then a targeted benchmark subset.

## Priority Order

1. Completion-path synchronization cost
2. Runtime child / TaskGroup fast path
3. Flow build and snapshot cost
4. Algorithm and benchmark-shape parity
5. Pipeline structure

## Task 1: Remove Completion-Path Locking From Hot Async Waits

**Why first:** This is the clearest remaining hot-path lock in the executor and directly affects `async_task`, `thread_pool`, and all tiny-DAG benchmarks that end in `run(...).wait()` or `wait_for_all()`.

**Target benchmarks:**

- `async_task`
- `thread_pool`
- `linear_chain`
- `embarrassing_parallelism`
- secondary: `binary_tree`

**Files:**

- Modify: `flow-core/src/executor.rs`
- Modify: `flow-core/src/async_handle.rs`
- Verify: `flow-core/tests/asyncs.rs`
- Verify: `flow-core/tests/basics.rs`

**Design:**

- Keep queue submission unchanged.
- Replace the `RunHandle` waiter path based on `Mutex + Condvar` with a lighter completion state.
- Remove `RunCompletionEvent(Mutex<u64> + Condvar)` from `wait_for_all`.
- Preserve `RunHandle::wait`, `RunHandle::cancel`, and current error semantics.

**Steps:**

1. Add a focused note in `flow-core/src/executor.rs` above `RunHandleState` documenting the current hot locks and the intended replacement.
2. Replace `RunHandleState.waiter` and `RunCompletionEvent` with a lighter event mechanism.
3. Keep a fast uncontended read path for `is_finished()` and `wait()`.
4. Re-run:
   - `cargo test -p flow-core basics::wait_for_all_blocks_until_all_runs_finish -- --exact`
   - `cargo test -p flow-core asyncs::silent_async_is_tracked_by_wait_for_all -- --exact`
5. Benchmark:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases async_task,thread_pool,linear_chain,embarrassing_parallelism,binary_tree --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task1`

**Exit criteria:**

- `async_task` small sizes improve materially.
- `thread_pool` ratio drops measurably.
- No regression in `RunHandle` correctness tests.

## Task 2: Introduce a True Runtime Child Fast Path

**Why second:** `fibonacci` is still very red, and `TaskGroup` currently still routes through executor async submission instead of a parent-linked lightweight child path.

**Target benchmarks:**

- `fibonacci`
- `nqueens`
- secondary: `integrate`

**Files:**

- Modify: `flow-core/src/executor.rs`
- Modify: `flow-core/src/runtime.rs`
- Modify: `flow-core/src/task_group.rs`
- Modify: `flow-core/src/async_handle.rs`
- Verify: `flow-core/tests/runtimes.rs`

**Design:**

- Keep `RuntimeJoinScope` atomic.
- Separate "runtime child with no handle" from generic `RunHandle` completion.
- Make `TaskGroup::silent_async` submit through the same lighter parent-linked path instead of `executor.silent_async`.
- Keep `runtime_async` value-returning path lightweight and worker-local where possible.

**Steps:**

1. Add an internal runtime-child submission helper in `flow-core/src/executor.rs` that does not allocate a `RunHandle`.
2. Route `RuntimeCtx::silent_async` and `RuntimeCtx::silent_result_async` through that helper.
3. Change `TaskGroup::silent_async` to reuse the same helper instead of `Executor::silent_async`.
4. Audit panic and cancellation propagation in both runtime and task-group paths.
5. Re-run:
   - `cargo test -p flow-core runtimes -- --nocapture`
6. Benchmark:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases fibonacci,nqueens,integrate --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task2`

**Exit criteria:**

- `fibonacci` improves materially from the current `12.36x @ 20`.
- `integrate` remains green.
- `TaskGroup` no longer depends on `Executor::silent_async` in the hot path.

## Task 3: Reduce Flow Build And Snapshot Cost

**Why third:** The queue is already lock-free, but repeated DAG-building benchmarks still pay `RwLock<Graph>` mutation and full `snapshot()` cloning every run.

**Target benchmarks:**

- `linear_chain`
- `binary_tree`
- `thread_pool`
- `wavefront`
- secondary: `graph_traversal`

**Files:**

- Modify: `flow-core/src/flow.rs`
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/basics.rs`
- Verify: `flow-core/tests/composition.rs`
- Verify: `flow-core/tests/subflows.rs`

**Design:**

- First attack repeated `snapshot()` cloning with a generation-based cached snapshot.
- Only if needed after that, evaluate whether graph mutation still needs full `RwLock` coverage.
- Do not change flow mutation semantics and do not allow concurrent mutation while running.

**Steps:**

1. Add a generation counter and cached immutable `GraphSnapshot` inside `FlowInner`.
2. Invalidate the cache on every graph mutation path in `TaskHandle` and `FlowBuilder`.
3. Reuse the cached snapshot in `Flow::snapshot()` when the graph generation is unchanged.
4. Re-run:
   - `cargo test -p flow-core basics composition subflows -- --nocapture`
5. Benchmark:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases linear_chain,binary_tree,thread_pool,wavefront,graph_traversal --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task3`

**Exit criteria:**

- Repeated-run DAG benchmarks improve without API changes.
- No graph validation or composition regressions.

## Task 4: Fix Benchmark-Shape And Algorithm Adapter Mismatches

**Why fourth:** Some red cases are not pure scheduler losses. RustFlow is still paying extra allocation or adapter costs compared to the original Taskflow benchmark shape.

**Target benchmarks:**

- `mandelbrot`
- `matrix_multiplication`
- `merge_sort`
- `black_scholes`
- secondary: `for_each`

**Files:**

- Modify: `flow-algorithms/src/reduce_transform.rs`
- Modify: `benchmarks/src/bin/taskflow_compare.rs`
- Verify: `flow-algorithms/tests/*`

**Design:**

- Introduce direct-write helpers where RustFlow currently returns per-row `Vec`s and then folds them.
- Separate "library improvement" from "benchmark adapter parity" in commits.
- Prefer preallocated destination buffers when Taskflow benchmark writes in-place.

**Steps:**

1. Add `parallel_transform_into` or equivalent internal helper in `flow-algorithms/src/reduce_transform.rs`.
2. Rework the RustFlow side of:
   - `mandelbrot`
   - `matrix_multiplication`
   to write into preallocated output rather than return `Vec<Vec<_>>`.
3. Review `merge_sort` in `benchmarks/src/bin/taskflow_compare.rs` and remove `Mutex<Option<Vec<_>>>` benchmark-only result passing if a lighter path is possible.
4. Re-run:
   - `cargo test -p flow-algorithms -- --nocapture`
5. Benchmark:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases mandelbrot,matrix_multiplication,merge_sort,black_scholes,for_each --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task4`

**Exit criteria:**

- The red algorithm cases improve without regressing current green algorithm cases.
- Benchmark report clearly distinguishes adapter improvements from core executor improvements.

## Task 5: Finish Scan/Sort Structural Work

**Why fifth:** The recent scan/sort work helped, but `scan` and `sort` are still materially behind at large sizes.

**Target benchmarks:**

- `scan`
- `sort`
- secondary: `primes`, `reduce_sum`

**Files:**

- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Modify: `flow-algorithms/src/parallel_for.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`

**Design:**

- Keep current lock-free direct-write scan buffer.
- Remove the outer runtime-task coordination shape when a flatter worker-aligned form is possible.
- Keep sort on as few allocations and merge rounds as possible.

**Steps:**

1. Rework scan coordination so the runtime layer does not add an avoidable extra scheduling shell.
2. Review the barrier path and replace pure spin where cooperative waiting is cheaper.
3. Reduce sort merge allocations and avoid extra async rounds for small remaining chunk counts.
4. Re-run:
   - `cargo test -p flow-algorithms find_scan_sort -- --nocapture`
5. Benchmark:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases scan,sort,primes,reduce_sum --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task5`

**Exit criteria:**

- `scan` ratio drops materially from the current `6.31x @ 100000`.
- `sort` ratio drops materially from the current `3.15x @ 100000`.

**Current status (2026-04-05):**

- Implemented:
  - flattened `scan` execution from `Flow + runtime task` into executor-root async fanout
  - added cooperative `corun_until` helpers on `Executor` and `RuntimeCtx`
  - replaced `parallel_sort_by` chunk-sort + repeated merge allocation rounds with in-place recursive partition sort
- Correctness:
  - fixed a real bug in `parallel_exclusive_scan`'s parallel path; previous small tests never exercised it because the scan cutoff kept them on the sequential path
  - added large-input tests to force the parallel scan/sort paths
- Verification:
  - `cargo test -p flow-algorithms --test find_scan_sort -- --nocapture`
  - `cargo test -p flow-core --test runtimes -- --nocapture`
  - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases scan,sort,primes,reduce_sum --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task5`
  - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 5 --cases scan,sort --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task5_rounds5`
- Outcome:
  - benchmark harness was also corrected so RustFlow no longer times random input generation in `scan`/`sort`, matching Taskflow's timing scope more closely
  - `scan` looks effectively recovered after the timing fix and scan flattening:
    - 5-round recheck `@10000`: `0.018 ms / 0.819x`
    - 5-round recheck `@100000`: `0.110 ms / 1.072x`
  - `sort` improved materially but is still behind at the largest point:
    - 5-round recheck `@10000`: `0.084 ms / 0.818x`
    - 5-round recheck `@100000`: `1.067 ms / 1.763x`
  - `primes` and `reduce_sum` were not the main target of this task and remained noisy across short runs
  - Task 5 should now focus on `sort`, not `scan`

## Task 6: Remove Remaining Pipeline Structural Overhead

**Why last:** Pipeline work has improved already, but it is still structurally heavier than Taskflow and touches more code paths.

**Target benchmarks:**

- `data_pipeline`
- `graph_pipeline`
- `linear_pipeline`
- secondary: `wavefront`

**Files:**

- Modify: `flow-core/src/pipeline.rs`
- Verify: `flow-core/tests/pipelines.rs`
- Verify: `flow-core/tests/scalable_data_pipelines.rs`

**Design:**

- Keep the current spinlock improvement.
- Reduce line-runner shell overhead and serial-stage bookkeeping.
- Re-evaluate whether typed data stages can avoid some `Arc<dyn ...>` overhead in steady state.

**Steps:**

1. Profile `run_pipe_chain` and `run_data_chain` structure for avoidable per-line overhead.
2. Reduce work repeated on every token claim in `run_pipe_line` and `run_data_line`.
3. If needed, specialize the typed data path before touching public API.
4. Re-run:
   - `cargo test -p flow-core pipelines scalable_data_pipelines -- --nocapture`
5. Benchmark:
   - `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline,wavefront --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task6`

**Exit criteria:**

- `data_pipeline` and `graph_pipeline` both improve materially.
- `linear_pipeline` does not regress.

## Reporting Cadence

After each task:

1. Capture the targeted test commands and results.
2. Capture the targeted benchmark report path.
3. Record:
   - improved cases
   - unchanged cases
   - regressed cases
4. Only then move to the next task.

## Recommended Execution Order

1. Task 1: completion-path locking
2. Task 2: runtime child / TaskGroup fast path
3. Task 3: flow build and snapshot cost
4. Task 4: adapter parity for red algorithm cases
5. Task 5: scan/sort structural work
6. Task 6: pipeline structure
