# Executor Worker Sleep/Wake Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace RustFlow's broadcast-heavy worker sleep/wake path with a two-phase worker parking design that lowers recursive runtime fixed cost without regressing correctness.

**Architecture:** Split executor waiting into two domains. Worker parking becomes a per-worker two-phase protocol (`prepare -> cancel|commit`) used only by the background worker loop, while cooperative inline runtime waits keep spinning/stealing without parking. `wait_for_all` remains a separate completion path and must not depend on worker parker state.

**Tech Stack:** Rust stable, `flow-core`, executor work-stealing queues, `std::sync::{Mutex, Condvar}`, workspace tests, `benchmarks` taskflow comparison harness.

## Progress Update (2026-04-03)

- Tasks 1-5 are implemented in `flow-core/src/executor.rs`.
- Worker sleep/wake now uses a two-phase per-worker atomic parker:
  - `prepare -> cancel|commit`
  - background workers block via `thread::park`
  - wakeups use targeted `Thread::unpark`
- `wait_until_inline` remains non-parking.
- `wait_for_all` is split onto a dedicated `RunCompletionEvent`.
- Enqueue wakeups are now edge-triggered on queue `empty -> non-empty` transitions to avoid repeated wake checks on large root fanouts.
- Root-heavy submissions now batch-distribute work instead of issuing one enqueue per root task.
- `GraphSnapshot` now caches root indices so repeated runs avoid rescanning the graph entry set.
- Added regression coverage in `flow-core/tests/runtimes.rs`:
  - `runtime_corun_children_does_not_deadlock_under_repeated_batches`

Current verification status:

- `cargo test -p flow-core -- --nocapture`: PASS
- `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases async_task,thread_pool,fibonacci,nqueens,integrate --output-dir benchmarks/reports/taskflow_compare/phase2_worker_sleep_wake_r3`: PASS, no hang

Current benchmark readout:

- Versus `phase2_runtime_join_scope_scoped_r3`, the runtime-heavy core set improved:
  - `Fibonacci` total RustFlow time: `13.468 ms -> 10.800 ms` (`-19.8%`)
  - `Integrate` total RustFlow time: `12.775 ms -> 9.686 ms` (`-24.2%`)
  - `N-Queens` total RustFlow time: `14.576 ms -> 4.920 ms` (`-66.2%`)
- Residual issue:
  - `Thread Pool` narrowed but is still slower than the archived `current_head_20260402_r3` report in the mixed-case `r3` run (`41.404 ms` vs `37.847 ms` at size `100`, about `+10.4%`).
  - A thread-pool-only probe lands closer to parity (`39.363 ms` at size `100`), which suggests the remaining gap is now much smaller and may be partly benchmark-noise-sensitive.

---

## Context

This plan refines the next step after Phase 2 runtime join-scope work:

- Best current Phase 2 report:
  - `benchmarks/reports/taskflow_compare/phase2_runtime_join_scope_scoped_r3/taskflow_vs_rustflow_report.md`
- Relevant implementation files:
  - `flow-core/src/executor.rs`
  - `flow-core/src/runtime.rs`
  - `flow-core/tests/runtimes.rs`
  - `flow-core/tests/asyncs.rs`
  - `flow-core/tests/observers.rs`
- Reference implementation:
  - `taskflow/taskflow/core/executor.hpp`
  - `taskflow/taskflow/core/atomic_notifier.hpp`

## Guardrails

- Keep the public executor/runtime API unchanged.
- Do not change work-stealing queue structure in this round.
- Do not let `wait_until_inline` park the current worker.
- Preserve cancellation, panic propagation, and observer semantics.
- Preserve current wins on `integrate`.
- Treat any benchmark hang as a blocker and revert immediately.

## Task 1: Isolate Driving Modes

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/runtimes.rs`
- Verify: `flow-core/tests/basics.rs`

**Step 1: Add an explicit drive mode**

Add small internal enums/helpers near the executor wait helpers:

```rust
enum DriveMode {
    AllowPark,
    NoPark,
}
```

```rust
fn drive_worker_until<F>(
    &self,
    worker_id: usize,
    mode: DriveMode,
    done: F,
) where
    F: FnMut() -> bool
```

**Step 2: Route existing callers through the helper**

- `worker_loop` uses `DriveMode::AllowPark`
- `wait_until_inline` uses `DriveMode::NoPark`

Keep behavior otherwise identical in this task. No new wakeup policy yet.

**Step 3: Run focused regression**

Run:

```bash
cargo test -p flow-core --test runtimes runtime_async_can_wait_inline_on_a_single_worker -- --exact --nocapture
cargo test -p flow-core --test basics wait_for_all_blocks_until_all_runs_finish -- --exact --nocapture
```

Expected: both PASS.

**Step 4: Run the surrounding suite**

Run:

```bash
cargo test -p flow-core --test runtimes -- --nocapture
```

Expected: PASS.

## Task 2: Add Per-Worker Parker State

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/observers.rs`

**Step 1: Add a parker record per worker**

Add an internal struct near `WorkerQueue`:

```rust
struct WorkerParker {
    state: Mutex<WorkerParkState>,
    ready: Condvar,
}
```

```rust
struct WorkerParkState {
    preparing: bool,
    parked: bool,
    epoch: u64,
}
```

Add `parkers: Vec<Arc<WorkerParker>>` to `ExecutorInner`.

**Step 2: Add parker helpers**

Add internal methods with exact ownership boundaries:

```rust
fn prepare_park(&self, worker_id: usize) -> u64
fn cancel_park(&self, worker_id: usize, observed_epoch: u64)
fn commit_park(&self, worker_id: usize, observed_epoch: u64, shutdown: &AtomicBool)
```

For this task, these methods may still be implemented with plain `Mutex + Condvar`; correctness first.

**Step 3: Keep observer callbacks aligned to actual park transitions**

- `notify_worker_sleep` fires only right before `commit_park`
- `notify_worker_wake` fires only after `commit_park` returns and shutdown is false

**Step 4: Run observer regression**

Run:

```bash
cargo test -p flow-core --test observers observer_receives_task_and_worker_lifecycle_events -- --exact --nocapture
```

Expected: PASS.

## Task 3: Implement Two-Phase Worker Parking

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/asyncs.rs`
- Verify: `flow-core/tests/runtimes.rs`

**Step 1: Add a pre-park recheck path**

Inside `DriveMode::AllowPark`, after local pop and steal both fail:

1. call `prepare_park(worker_id)`
2. recheck local queue
3. recheck steal path
4. recheck shutdown
5. if any condition changed, call `cancel_park(...)`
6. otherwise call `commit_park(...)`

This is the key lost-wakeup prevention step. Do not skip the recheck.

**Step 2: Keep inline waits non-parking**

Inside `DriveMode::NoPark`, replace blocking wait with:

- continuation take
- local pop
- steal
- bounded `thread::yield_now()`
- re-evaluate predicate

Do not call `commit_park` from inline runtime waits.

**Step 3: Add one narrow deadlock regression**

Extend `flow-core/tests/runtimes.rs` with a test that repeatedly spawns runtime silent children and waits with `corun_children()` on multiple workers.

Target helper shape:

```rust
#[test]
fn runtime_corun_children_does_not_deadlock_under_repeated_batches() { ... }
```

**Step 4: Run targeted regression**

Run:

```bash
cargo test -p flow-core --test asyncs silent_async_is_tracked_by_wait_for_all -- --exact --nocapture
cargo test -p flow-core --test runtimes -- --nocapture
```

Expected: PASS, no hangs.

## Task 4: Narrow Wakeups by Task Volume

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/basics.rs`
- Verify: `flow-core/tests/asyncs.rs`

**Step 1: Add wake helpers**

Add internal methods:

```rust
fn wake_one_worker(&self)
fn wake_n_workers(&self, n: usize)
fn wake_all_workers(&self)
```

Implementation rule:

- single enqueue: `wake_one_worker`
- bulk waiter release: `wake_n_workers(waiters.len())`
- shutdown: `wake_all_workers`

**Step 2: Keep run completion separate**

Do not reuse worker wake helpers for `wait_for_all` completion. That path stays on run completion signaling.

**Step 3: Run correctness regression**

Run:

```bash
cargo test -p flow-core --test basics -- --nocapture
cargo test -p flow-core --test asyncs -- --nocapture
```

Expected: PASS.

## Task 5: Split Run Completion from Worker Parking

**Files:**
- Modify: `flow-core/src/executor.rs`
- Verify: `flow-core/tests/basics.rs`
- Verify: `flow-core/tests/asyncs.rs`

**Step 1: Introduce a dedicated completion event**

Add a small internal type:

```rust
struct RunCompletionEvent {
    epoch: Mutex<u64>,
    ready: Condvar,
}
```

Move `wait_for_all` and `active_runs == 0` completion notification onto this type.

**Step 2: Remove worker-park coupling from `wait_for_all`**

`Executor::wait_for_all` must wait only on `active_runs` completion, not on worker parker state.

**Step 3: Run focused regression**

Run:

```bash
cargo test -p flow-core --test basics wait_for_all_blocks_until_all_runs_finish -- --exact --nocapture
cargo test -p flow-core --test asyncs silent_async_is_tracked_by_wait_for_all -- --exact --nocapture
```

Expected: PASS.

## Task 6: Full Verification and Benchmark Gate

**Files:**
- Verify: `flow-core`
- Verify: `benchmarks/src/bin/taskflow_compare.rs`

**Step 1: Run full core regression**

Run:

```bash
cargo test -p flow-core -- --nocapture
```

Expected: PASS.

**Step 2: Run executor/runtime benchmark subset**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 1 --cases async_task,thread_pool,fibonacci,nqueens,integrate --output-dir benchmarks/reports/taskflow_compare/phase2_worker_sleep_wake_r1
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases async_task,thread_pool,fibonacci,nqueens,integrate --output-dir benchmarks/reports/taskflow_compare/phase2_worker_sleep_wake_r3
```

**Step 3: Acceptance criteria**

- `fibonacci` absolute RustFlow time improves against `phase2_runtime_join_scope_scoped_r3`
- `integrate` does not regress materially
- no benchmark hangs
- `thread_pool` and `async_task` do not regress materially

**Step 4: If benchmarks hang**

Stop. Capture:

- last completed case printed by `taskflow_compare`
- current `ps` entry for the benchmark process
- whether output directory/report file was created

Revert the last wakeup-policy step before continuing.
