# Post-Lockfree Queue Progress Report (2026-04-05)

## Scope

This report captures implementation and benchmark progress after the lock-free queue migration, based on the current local workspace state as of 2026-04-05.

## Summary

- Tasks 1-4 are completed and landed in local history.
- Task 5 (scan/sort) is partially completed:
  - `scan` is now close to parity.
  - `sort` improved materially but still trails Taskflow at large size.
- Task 6 (pipeline structure) has started, but no clear material benchmark win yet.
- A semantics-regressing pipeline scheduling experiment was identified by tests and reverted.
- A lighter downstream serial-stage turn-counter experiment was also validated for correctness, but benchmark rechecks did not beat the stage-gate checkpoint and were rolled back from the working tree.

## Landed Baseline Commit

- Commit: `bda2a26`
- Message: `Post-lockfree perf pass: task2-5 runtime/taskgroup and scan-sort updates`

This commit includes:
- doc cleanup and consolidation
- task2/task3/task4 runtime and benchmark-shape work
- task5 first-pass scan/sort structural updates and benchmark reports

## Current Uncommitted Work

Files with active local updates:

- `flow-algorithms/src/find_scan_sort.rs`
- `benchmarks/src/bin/taskflow_compare.rs`
- `flow-core/src/pipeline.rs`
- `docs/plans/2026-04-05-post-lockfree-queue-performance-plan.md`
- `benchmarks/reports/taskflow_compare/post_lockfree_queue_task5_rounds5/taskflow_vs_rustflow_report.md`
- `benchmarks/reports/taskflow_compare/post_lockfree_queue_task5_sort_tune/*`
- `benchmarks/reports/taskflow_compare/post_lockfree_queue_task6/*`

## Task 5 Status (Scan/Sort)

### Implemented

- Flattened scan execution path to reduce wrapper overhead.
- Added cooperative wait path support in executor/runtime hot paths.
- Reworked sort path and tuned cutoffs for lower overhead.
- Corrected benchmark timing scope for RustFlow `scan`/`sort`:
  - random input generation is no longer included in timed section.

### Verification

- `cargo test -p flow-algorithms --test find_scan_sort -- --nocapture`
- `cargo test -p flow-core --test runtimes -- --nocapture`
- repeated targeted benchmark runs on `scan,sort`

### Latest 5-round Result

Source:
- `benchmarks/reports/taskflow_compare/post_lockfree_queue_task5_rounds5/taskflow_vs_rustflow_report.md`

Key points:
- `scan @100000`: RustFlow `0.112 ms`, Taskflow `0.102 ms`, ratio `1.096x`
- `sort @100000`: RustFlow `0.949 ms`, Taskflow `0.611 ms`, ratio `1.554x`

Conclusion:
- `scan` is effectively near parity.
- `sort` remains the main residual gap in Task 5.

## Task 6 Status (Pipeline Structure)

### Implemented

- Replaced pipeline token-count bookkeeping from `Mutex<usize>` to `AtomicUsize` in pipeline internals.

### Tried and Reverted

- Replaced line-worker scheduling with `TaskGroup` fast path in pipeline runners.
- This introduced token-accounting regressions in scalable/data pipeline tests (off-by-one token count).
- The scheduling change was reverted to preserve semantics and test correctness.

### Verification

- `cargo test -p flow-core --test pipelines -- --nocapture`
- `cargo test -p flow-core --test scalable_data_pipelines -- --nocapture`
- `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/post_lockfree_queue_task6`
- `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_baseline`
- `cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases data_pipeline,graph_pipeline,linear_pipeline --output-dir benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_gate`

### Latest Task 6 Benchmark Snapshot

Source:
- `benchmarks/reports/taskflow_compare/pipeline_hotpath_baseline/taskflow_vs_rustflow_report.md`

At largest points:
- `data_pipeline @1024`: ratio `6.737x`
- `graph_pipeline @12904`: ratio `4.911x`
- `linear_pipeline @1024`: ratio `3.706x`

Conclusion:
- No clear material Task 6 win yet.
- Frozen restart point for the next pass is `pipeline_hotpath_baseline`.
- Further work should focus on serial-stage contention, stage progression, and line scheduling hot paths while preserving current semantics.

### Current Restart Checkpoint

- Landed in the working tree:
  - all-serial pipeline regression coverage in `flow-core/tests/pipelines.rs`
  - all-serial data/scalable pipeline regression coverage in `flow-core/tests/scalable_data_pipelines.rs`
  - contention-friendly serial-stage gate using bounded spin + `yield_now` in `flow-core/src/pipeline.rs`
  - runtime stop vs abort split in pipeline run state so normal stage-0 stop halts token issuance without treating the run like an internal failure
- Measured effect versus the frozen baseline:
  - `data_pipeline @1024`: `6.737x -> 4.416x`
  - `graph_pipeline @12904`: `4.911x -> 4.917x` (effectively flat)
  - `linear_pipeline @1024`: `3.706x -> 3.731x` (effectively flat)
- Rejected experiment:
  - rescheduling blocked line progress as fresh runtime children increased task churn and regressed RustFlow wall times; the change was rolled back after focused tests and `pipeline_hotpath_stage_progress` benchmarking

### Additional Task 6 Experiments (2026-04-05)

- Focused correctness remained green throughout:
  - `cargo test -p flow-core --test pipelines -- --nocapture`
  - `cargo test -p flow-core --test scalable_data_pipelines -- --nocapture`
- Data pipeline hot-path pass 1 in `flow-core/src/pipeline.rs`:
  - changed `DataPipelineInner` to hold prebuilt `Arc<[DataStage]>` stage storage, removing per-run stage-vector cloning
  - replaced per-line `LineSlot` initialized atomics with line-owned flags, preserving semantics while removing same-line atomic traffic from the steady state
  - focused verification:
    - `cargo test -p flow-core --test pipelines -- --nocapture`
    - `cargo test -p flow-core --test scalable_data_pipelines -- --nocapture`
  - reports:
    - `benchmarks/reports/taskflow_compare/data_pipeline_hotpath_pass1/taskflow_vs_rustflow_report.md`
    - `benchmarks/reports/taskflow_compare/data_pipeline_hotpath_pass1_recheck/taskflow_vs_rustflow_report.md`
    - `benchmarks/reports/taskflow_compare/data_pipeline_hotpath_pass1_rounds5/taskflow_vs_rustflow_report.md`
  - largest-point readings:
    - 3-round pass 1:
      - `data_pipeline @1024`: `11.224 ms` vs Taskflow `2.701 ms`, ratio `4.155x`
      - `linear_pipeline @1024`: `121.686 ms` vs Taskflow `32.969 ms`, ratio `3.691x`
    - 3-round recheck:
      - `data_pipeline @1024`: `11.283 ms` vs Taskflow `2.533 ms`, ratio `4.454x`
      - `linear_pipeline @1024`: `121.964 ms` vs Taskflow `32.979 ms`, ratio `3.698x`
    - 5-round stabilizing run:
      - `data_pipeline @1024`: `10.474 ms` vs Taskflow `2.372 ms`, ratio `4.415x`
      - `linear_pipeline @1024`: `121.911 ms` vs Taskflow `32.935 ms`, ratio `3.702x`
  - readout:
    - `data_pipeline` absolute RustFlow time improved materially versus the stage-gate checkpoint (`11.43 ms -> 10.47 ms` in the 5-round run)
    - ratio-level comparisons remain noisy because the local Taskflow side also moves between runs, but the RustFlow-side reduction is repeatable enough to keep this pass
    - `linear_pipeline` stayed flat to slightly better, so the `DataPipeline` changes do not appear to regress the non-data pipeline path
- Graph pipeline benchmark-parity pass 1 in `benchmarks/src/bin/taskflow_compare.rs`:
  - moved RustFlow-side `graph_pipeline` checksum out of the timed region to match the Taskflow driver timing scope
  - replaced benchmark-only `AtomicI32` line buffers and node-value storage with plain integer storage, matching the Taskflow fixture shape more closely
  - reports:
    - `benchmarks/reports/taskflow_compare/graph_pipeline_parity_pass1/taskflow_vs_rustflow_report.md`
    - `benchmarks/reports/taskflow_compare/graph_pipeline_parity_pass1_recheck/taskflow_vs_rustflow_report.md`
  - `graph_pipeline @12904` ratios:
    - baseline: `4.911x`
    - parity pass 1: `4.844x`
    - parity pass 1 recheck: `4.883x`
  - readout:
    - benchmark-side atomics and timed checksum were real overhead, especially at smaller graph sizes
    - largest-point improvement was only marginal, so the remaining large-size graph gap is still dominated by library/runtime behavior rather than benchmark harness skew
- Rejected static `Pipeline` storage pass:
  - tried prebuilding static `Pipeline` pipe storage as a shared slice to avoid per-run `Vec<Pipe>` cloning
  - report from the attempted version:
    - `benchmarks/reports/taskflow_compare/pipeline_static_hotpath_pass1/taskflow_vs_rustflow_report.md`
  - rollback confirmation report:
    - `benchmarks/reports/taskflow_compare/pipeline_static_hotpath_rollback_recheck/taskflow_vs_rustflow_report.md`
  - readout:
    - the attempted version did not produce a reliable graph win and showed materially worse `linear_pipeline` readings
    - the change was rolled back; current direction should continue favoring `DataPipeline`-specific storage/handoff reductions over generic static-pipeline reshaping
- Lighter downstream serial-stage turn-counter variant:
  - report: `benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_turns/taskflow_vs_rustflow_report.md`
  - largest-point ratios:
    - `data_pipeline @1024`: `6.681x`
    - `graph_pipeline @12904`: `4.752x`
    - `linear_pipeline @1024`: `3.928x`
- Turn-counter with cheaper `yield_now` waiting:
  - report: `benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_turns_yield/taskflow_vs_rustflow_report.md`
  - largest-point ratios:
    - `data_pipeline @1024`: `5.265x`
    - `graph_pipeline @12904`: `5.109x`
    - `linear_pipeline @1024`: `3.725x`
- Outcome:
  - both turn-counter variants preserved stop/reset accounting but failed to produce a robust all-case win over the stage-gate checkpoint
  - the working tree was rolled back to the stage-gate-style implementation instead of keeping the turn-counter path
- Final current-code rechecks:
  - `benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_gate_recheck/taskflow_vs_rustflow_report.md`
  - `benchmarks/reports/taskflow_compare/pipeline_hotpath_stage_gate_recheck2/taskflow_vs_rustflow_report.md`
  - largest-point ratios across the two rechecks:
    - `data_pipeline @1024`: `3.953x` and `4.289x`
    - `graph_pipeline @12904`: `5.454x` and `5.475x`
    - `linear_pipeline @1024`: `3.696x` and `3.699x`
- Readout:
  - `data_pipeline` remains the clearest beneficiary of the stage-gate work
  - `linear_pipeline` is effectively flat to slightly better than baseline
  - `graph_pipeline` remains noisy and is not improved by the lighter Task 3 line, so Task 6 should not continue with downstream turn-based scheduling in the current design

## Current Bottlenecks

1. `sort` large-size gap (`@100000`) remains the largest residual in Task 5.
2. Pipeline family (`data_pipeline`, `graph_pipeline`, `linear_pipeline`) remains significantly behind Taskflow.

## Recommended Next Execution Order

1. Continue Task 5 with focused `sort @100000` optimization.
2. Resume Task 6 only with smaller hot-loop work such as `DataPipeline` dispatch/storage overhead trimming and benchmark-parity cleanup; avoid further downstream turn-counter rewrites unless the execution model changes.
3. Re-run targeted benchmark subsets after each small change and keep regression tests mandatory.
