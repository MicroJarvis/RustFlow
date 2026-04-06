# Pipeline Hotpath Optimization Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce the RustFlow vs Taskflow gap in `data_pipeline`, `graph_pipeline`, and `linear_pipeline` by removing the highest-value hot-path costs without regressing pipeline semantics.

**Architecture:** Keep the public `Pipeline`, `ScalablePipeline`, and `DataPipeline` APIs unchanged. Optimize in priority order: first fix the common serial-stage contention path, then reduce stage-progression worker blocking, then trim `DataPipeline`-specific steady-state overhead, and only after that do benchmark-parity cleanup for `graph_pipeline`.

**Tech Stack:** Rust stable, `flow-core`, `benchmarks`, workspace tests, local Taskflow benchmark fixtures under `taskflow/`.

---

## Context

Latest local benchmark recheck on 2026-04-05:

- command:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/current_pipeline_check_20260405
```

- largest-point ratios:
  - `data_pipeline @1024`: `4.001x`
  - `graph_pipeline @12904`: `5.314x`
  - `linear_pipeline @1024`: `3.808x`

Primary evidence from the current code:

- serial stages still use pure busy-spin locking in `flow-core/src/pipeline.rs`
- line runners block a worker while waiting for a serial stage instead of yielding cooperatively
- `DataPipeline` still pays per-stage dynamic dispatch and `LineSlot` handoff cost
- `graph_pipeline` benchmark uses extra Rust-side atomics not present in Taskflow's benchmark fixture

Optimization order for this plan:

1. Freeze the current baseline and guardrails
2. Replace pure-spin serial stage waiting with a contention-friendly gate
3. Rework stage progression so blocked lines yield instead of burning a worker
4. Trim `DataPipeline` steady-state overhead
5. Fix `graph_pipeline` benchmark parity separately from library-core work
6. Re-run focused benchmarks and update docs

## Task 1: Freeze The Pipeline Baseline And Guardrails

**Files:**
- Create: `benchmarks/reports/taskflow_compare/pipeline_hotpath_baseline/taskflow_vs_rustflow_report.md`
- Modify: `docs/plans/2026-04-05-post-lockfree-progress-report.md`

**Step 1: Re-run the focused pipeline tests first**

Run:

```bash
cargo test -p flow-core --test pipelines -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test scalable_data_pipelines -- --nocapture
```

Expected: PASS

**Step 2: Re-capture the current benchmark baseline**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_baseline
```

Expected: report written under `benchmarks/reports/taskflow_compare/pipeline_hotpath_baseline`

**Step 3: Update the progress report with the exact baseline ratios**

Record:

- `data_pipeline @1024`
- `graph_pipeline @12904`
- `linear_pipeline @1024`

Keep this note separate from any future benchmark-parity edits.

**Step 4: Commit**

```bash
git add benchmarks/reports/taskflow_compare/pipeline_hotpath_baseline docs/plans/2026-04-05-post-lockfree-progress-report.md
git commit -m "bench: freeze pipeline hotpath baseline"
```

## Task 2: Replace Pure Spin With A Contention-Friendly Serial Stage Gate

**Why second:** This is the clearest common bottleneck across `Pipeline` and `DataPipeline`. Today a line runner can occupy a worker and spin for the full duration of a serial stage body.

**Files:**
- Modify: `flow-core/src/pipeline.rs`
- Test: `flow-core/tests/pipelines.rs`
- Test: `flow-core/tests/scalable_data_pipelines.rs`

**Step 1: Add regression tests before touching the hot path**

Add one focused test in `flow-core/tests/pipelines.rs` that uses:

- `num_lines = 4`
- `8` serial pipes
- stop at token `8`
- an `AtomicUsize` counter per stage

Assert:

- `pipeline.num_tokens() == 8`
- every stage saw exactly `8` tokens
- no stage executes token `8`

Add one focused test in `flow-core/tests/scalable_data_pipelines.rs` that builds an all-serial `DataPipeline` with:

- source -> stage -> sink
- stop at token `8`

Assert:

- `pipeline.num_tokens() == 8`
- sink receives exactly `8` values in order

**Step 2: Add `try_lock` support and a bounded-spin helper**

Introduce the lock helpers in `flow-core/src/pipeline.rs`:

```rust
impl SpinLock {
    fn try_lock(&self) -> Option<SpinLockGuard<'_>> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| SpinLockGuard { locked: &self.locked })
    }

    fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }
}
```

Add a helper boundary:

```rust
fn acquire_serial_stage<'a>(
    runtime: &RuntimeCtx,
    stage_lock: &'a SpinLock,
    run_state: &PipelineRunState,
) -> Option<SpinLockGuard<'a>> {
    let mut spins = 0usize;

    loop {
        if let Some(guard) = stage_lock.try_lock() {
            return Some(guard);
        }

        if run_state.should_stop() {
            return None;
        }

        spins += 1;
        if spins < 64 {
            std::hint::spin_loop();
        } else {
            spins = 0;
            runtime.corun_until(|| run_state.should_stop() || !stage_lock.is_locked());
        }
    }
}
```

**Step 3: Thread `RuntimeCtx` through the hot loops**

Change:

- `run_pipe_line(...)`
- `run_data_line(...)`

to accept `&RuntimeCtx`, and use `acquire_serial_stage(...)` for every serial-stage gate instead of unconditional `lock()`.

**Step 4: Run focused correctness**

Run:

```bash
cargo test -p flow-core --test pipelines -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test scalable_data_pipelines -- --nocapture
```

Expected: PASS

**Step 5: Re-run the focused benchmark**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_gate
```

Expected:

- `linear_pipeline @1024` improves materially versus baseline
- `data_pipeline @1024` improves or stays flat
- no correctness regressions

**Step 6: Commit**

```bash
git add flow-core/src/pipeline.rs flow-core/tests/pipelines.rs flow-core/tests/scalable_data_pipelines.rs benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_gate
git commit -m "perf: add contention-friendly pipeline serial stage gate"
```

## Task 3: Yield Blocked Lines Instead Of Holding A Worker In The Stage Loop

**Why third:** Task 2 reduces contention cost, but the current design still keeps one long-lived runner per line. When a line cannot advance, that worker is still tied up in the line loop.

**Files:**
- Modify: `flow-core/src/pipeline.rs`
- Test: `flow-core/tests/pipelines.rs`
- Test: `flow-core/tests/scalable_data_pipelines.rs`

**Step 1: Add a regression test for token accounting under repeated stop/restart**

In `flow-core/tests/scalable_data_pipelines.rs`, add a test that:

- runs a `ScalablePipeline` with `8` serial pipes
- stops at token `16`
- resets and reruns the same pipeline

Assert:

- both runs report `num_tokens() == 16`
- total observed sink events equal `32`
- no off-by-one token appears after reset

**Step 2: Introduce an explicit line-progress state**

Add a local state struct in `flow-core/src/pipeline.rs`:

```rust
struct LineProgress {
    line: usize,
    token: usize,
    next_pipe: usize,
}
```

Add helpers shaped like:

```rust
fn drive_pipe_progress(
    runtime: &RuntimeCtx,
    pipes: &[Pipe],
    progress: &mut LineProgress,
    run_state: &PipelineRunState,
) -> Result<DriveResult, FlowError>
```

and

```rust
fn drive_data_progress(
    runtime: &RuntimeCtx,
    stages: &[DataStage],
    progress: &mut LineProgress,
    run_state: &PipelineRunState,
) -> Result<DriveResult, FlowError>
```

where `DriveResult` is one of:

- `CompletedToken`
- `BlockedOnSerialStage`
- `Stopped`

**Step 3: Rework the outer line runner into a resumable loop**

Replace the current `run_pipe_line` / `run_data_line` shape:

```rust
loop {
    claim token
    run stage 0
    for remaining stages { ... }
}
```

with:

```rust
loop {
    if progress.next_pipe == 0 {
        claim_new_token(...)?;
    }

    match drive_*_progress(runtime, ..., &mut progress, run_state)? {
        DriveResult::CompletedToken => {
            progress.next_pipe = 0;
        }
        DriveResult::BlockedOnSerialStage => {
            runtime.corun_until(|| run_state.should_stop());
        }
        DriveResult::Stopped => return Ok(()),
    }
}
```

The exact helper names can change, but the intent must remain: do not keep a worker stuck in a pure wait loop while another line owns the serial stage.

**Step 4: Run focused correctness**

Run:

```bash
cargo test -p flow-core --test pipelines -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test scalable_data_pipelines -- --nocapture
```

Expected: PASS

**Step 5: Re-run the focused benchmark**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_progress
```

Expected:

- `linear_pipeline @1024` improves again versus Task 2
- `data_pipeline @1024` improves again or at least no longer worsens with size
- token-accounting tests stay green

**Step 6: Commit**

```bash
git add flow-core/src/pipeline.rs flow-core/tests/pipelines.rs flow-core/tests/scalable_data_pipelines.rs benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_progress
git commit -m "perf: yield blocked pipeline lines during stage progression"
```

## Task 4: Trim `DataPipeline` Steady-State Overhead

**Why fourth:** This is the first task that is `DataPipeline`-specific. It should wait until the common `Pipeline` hot path has been improved.

**Files:**
- Modify: `flow-core/src/pipeline.rs`
- Test: `flow-core/tests/scalable_data_pipelines.rs`

**Step 1: Add a focused heterogeneous payload regression**

Extend `flow-core/tests/scalable_data_pipelines.rs` with one test that keeps:

- `String -> [usize; 8] -> usize`
- `num_lines = 4`
- stop at token `16`

Assert:

- every sink output is present
- token order remains correct

**Step 2: Remove avoidable hot-path branching around `pipe_type()`**

Split `DataStage` into:

```rust
struct DataStage {
    pipe_type: PipeType,
    runner: Arc<dyn DataStageRunner>,
}
```

and change all call sites to read `stage.pipe_type` directly instead of virtual `pipe_type()` calls in the stage-lock setup path.

**Step 3: Hoist stage metadata out of the token loop**

Precompute once per run:

```rust
struct StageMeta {
    serial: bool,
}
```

Use that metadata in both `run_data_chain` and the resumable progress helpers so the hot loop does not repeatedly branch through dynamic stage metadata.

**Step 4: Make `LineSlot` debug-heavy and release-light**

Keep correctness checks in tests, but move the expensive initialized-flag path behind debug-only assertions:

```rust
#[cfg(debug_assertions)]
initialized: Vec<AtomicBool>
```

In release builds, keep the same ownership discipline but avoid the per-token `AtomicBool::swap` path.

**Step 5: Run focused correctness**

Run:

```bash
cargo test -p flow-core --test scalable_data_pipelines -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test pipelines -- --nocapture
```

Expected: PASS

**Step 6: Re-run the data-pipeline benchmark**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_data_stage
```

Expected:

- `data_pipeline @1024` improves versus Task 3
- `linear_pipeline` does not need to be rechecked yet if no shared hot loop changed

**Step 7: Commit**

```bash
git add flow-core/src/pipeline.rs flow-core/tests/scalable_data_pipelines.rs benchmarks/reports/taskflow_compare/pipeline_hotpath_data_stage
git commit -m "perf: trim data pipeline steady-state overhead"
```

## Task 5: Fix `graph_pipeline` Benchmark Parity Separately

**Why fifth:** `graph_pipeline` currently mixes pipeline runtime cost with Rust-only atomic adapter overhead. This should not be conflated with library-core optimization work.

**Files:**
- Modify: `benchmarks/src/bin/taskflow_compare.rs`
- Verify: `taskflow/benchmarks/graph_pipeline/taskflow.cpp`

**Step 1: Add a parity note above the Rust benchmark body**

Document that Taskflow's benchmark uses:

- `std::vector<int>` for the line buffer
- direct graph node value reads/writes

while the current Rust side uses extra atomics.

**Step 2: Replace benchmark-only atomics with fixture-matching storage**

In `run_graph_pipeline_case`, replace:

- `Arc<Vec<AtomicI32>>` buffer
- `Arc<Vec<AtomicI32>>` values

with a benchmark-local structure closer to the Taskflow fixture:

```rust
struct GraphPipelineState {
    buffer: Vec<UnsafeCell<i32>>,
    values: Vec<UnsafeCell<i32>>,
}
```

Use per-line ownership for `buffer[line]` and graph-topology ordering for `values`.
Keep this change benchmark-local. Do not push benchmark-specific storage hacks into `flow-core`.

**Step 3: Re-run graph pipeline only**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases graph_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_graph_parity
```

Expected:

- `graph_pipeline @12904` improves versus Task 3
- the benchmark still validates and reports a checksum

**Step 4: Commit**

```bash
git add benchmarks/src/bin/taskflow_compare.rs benchmarks/reports/taskflow_compare/pipeline_hotpath_graph_parity
git commit -m "bench: align graph pipeline adapter with Taskflow fixture"
```

## Task 6: Close Out The Pipeline Pass

**Files:**
- Modify: `docs/plans/2026-04-05-post-lockfree-progress-report.md`
- Modify: `README.md`

**Step 1: Run the focused final validation set**

Run:

```bash
cargo test -p flow-core --test pipelines -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test scalable_data_pipelines -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test --workspace
```

Expected: PASS

**Step 2: Run the final benchmark closeout**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_closeout
```

Expected: report written under `benchmarks/reports/taskflow_compare/pipeline_hotpath_closeout`

**Step 3: Update docs with the final numbers and the remaining gap**

Record:

- before vs after ratios for the three pipeline cases
- which improvement came from `flow-core`
- which improvement came from benchmark-parity work
- whether a follow-up task is still justified

**Step 4: Commit**

```bash
git add README.md docs/plans/2026-04-05-post-lockfree-progress-report.md benchmarks/reports/taskflow_compare/pipeline_hotpath_closeout
git commit -m "docs: close out pipeline hotpath optimization pass"
```

## Exit Criteria

Stop this plan when one of these is true:

- `linear_pipeline @1024 <= 2.75x` and `data_pipeline @1024 <= 3.25x`, or
- two consecutive core-runtime iterations fail to improve the best `linear_pipeline` result, or
- benchmark-parity fixes are the only remaining material wins

## Risk Notes

- The biggest correctness risk is token accounting under stop/reset semantics.
- Do not merge stage-progression changes without `pipelines` and `scalable_data_pipelines` passing.
- Keep `graph_pipeline` benchmark parity changes in `benchmarks/` only.
- Do not mix benchmark-only storage shortcuts into the public pipeline API or `flow-core`.
