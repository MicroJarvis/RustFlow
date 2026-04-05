# Sort Closeout Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Freeze the `sort` benchmark contract, close the remaining `sort @100000` gap as far as practical, and update progress docs with a stable result before starting new pipeline optimization work.

**Architecture:** Keep the current public `parallel_sort` / `parallel_sort_by` API and the current runtime-child recursive structure. Treat the remaining work as two separate concerns: first stabilize the benchmark harness so numbers are comparable across runs, then tune or finish the internal sort implementation against that fixed contract.

**Tech Stack:** Rust stable, `flow-algorithms`, `flow-core`, `benchmarks`, local C++ Taskflow checkout under `taskflow/`, workspace tests, `taskflow_compare` benchmark harness.

---

## Context

This plan assumes the current local workspace state on 2026-04-05:

- `parallel_sort` no longer routes its main recursive path through `select_nth_unstable_by`
- the current sort path already has:
  - explicit pivot selection
  - explicit partitioning
  - bad-partition budget
  - heapsort fallback
  - equal-heavy and nearly-sorted handling
- correctness tests for large sorted, reverse-sorted, duplicate-heavy, all-equal, and nearly-sorted inputs already exist
- the remaining instability is mostly in:
  - benchmark contract drift
  - threshold tuning
  - the leaf fallback still using `slice.sort_unstable_by(...)`

Primary files in scope:

- `flow-algorithms/src/find_scan_sort.rs`
- `flow-algorithms/tests/find_scan_sort.rs`
- `flow-core/tests/runtimes.rs`
- `benchmarks/src/bin/taskflow_compare.rs`
- `docs/plans/2026-04-05-post-lockfree-progress-report.md`
- `README.md`

Guardrails:

- Do not change the public `parallel_sort` / `parallel_sort_by` API.
- Do not start pipeline structural work until the sort benchmark contract is frozen.
- Keep `RuntimeCtx::silent_async` + tail-recursive inline continuation + `runtime.corun_children()` intact unless a regression forces a change.
- Do not use short noisy 3-round sort numbers as the canonical conclusion.
- Keep `scan` near parity and avoid unexpected regressions in `merge_sort`.

Canonical sort commands for this plan:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 1 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads1
```

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads4
```

Regression subset command:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases scan,sort,merge_sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_regression_subset
```

## Task 1: Freeze The Sort Benchmark Contract

**Files:**
- Modify: `benchmarks/src/bin/taskflow_compare.rs`

**Step 1: Write the harness cleanup patch**

Make the sort input fill/timing boundary explicit so future edits do not silently change what is timed:

```rust
fn refill_sort_input_i32(buf: &mut [i32]) {
    for value in buf {
        *value = c_rand_value();
    }
}

fn run_sort_round(
    executor: &Executor,
    input: &mut Vec<i32>,
) -> Result<f64, String> {
    refill_sort_input_i32(input);
    let round_input = std::mem::take(input);
    let start = Instant::now();
    let output = parallel_sort(executor, round_input, ParallelForOptions::default())
        .map_err(|error| error.to_string())?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
    *input = output;
    Ok(elapsed_ms)
}
```

Add one short comment above the timed section explaining that input generation is intentionally outside the timing window for parity with Taskflow.

**Step 2: Run format and the targeted benchmark once**

Run:

```bash
cargo fmt --all
```

Expected: PASS

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_contract_check
```

Expected: a report is written under `benchmarks/reports/taskflow_compare/sort_contract_check`

**Step 3: Commit**

```bash
git add benchmarks/src/bin/taskflow_compare.rs benchmarks/reports/taskflow_compare/sort_contract_check
git commit -m "bench: freeze sort benchmark timing contract"
```

## Task 2: Capture Canonical Sort Baselines

**Files:**
- Create: `benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads1/taskflow_vs_rustflow_report.md`
- Create: `benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads4/taskflow_vs_rustflow_report.md`

**Step 1: Capture the 1-thread baseline**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 1 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads1
```

Expected: report exists and `sort @100000` is recorded for the fixed contract

**Step 2: Capture the 4-thread baseline**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads4
```

Expected: report exists and becomes the canonical multi-thread baseline for this closeout

**Step 3: Record the current numbers in the workpad or commit message draft**

Record:

- `threads=1, sort @100000`
- `threads=4, sort @100000`
- whether the 4-thread result is still in the `~1.5x` band or has already drifted

**Step 4: Commit**

```bash
git add benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads1 benchmarks/reports/taskflow_compare/sort_closeout_baseline_threads4
git commit -m "bench: capture canonical sort closeout baselines"
```

## Task 3: Tune Thresholds Before Changing The Algorithm Again

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`
- Verify: `flow-core/tests/runtimes.rs`

**Step 1: Make threshold constants easy to tune and document**

Group the threshold constants and add one short comment documenting their purpose:

```rust
// Sort tuning knobs. Keep these local until the benchmark contract is stable.
const SORT_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 32_768;
const SORT_INSERTION_SORT_THRESHOLD: usize = 24;
const SORT_PARTIAL_INSERTION_SORT_LIMIT: usize = 8;
const SORT_NINTHER_THRESHOLD: usize = 128;
const SORT_IMBALANCED_PARTITION_DIVISOR: usize = 8;
```

**Step 2: Change exactly one threshold at a time**

Start with this order:

1. `SORT_SEQUENTIAL_CUTOFF_PER_WORKER`
2. `SORT_INSERTION_SORT_THRESHOLD`
3. `SORT_NINTHER_THRESHOLD`
4. `initial_sort_bad_partition_budget`

Do not change two knobs in the same edit.

**Step 3: Run correctness after every threshold change**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test runtimes -- --nocapture
```

Expected: PASS

**Step 4: Re-run canonical sort benchmarks after every promising change**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 1 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_tune_threads1
```

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_tune_threads4
```

Expected:

- the 1-thread result does not regress materially
- the 4-thread `sort @100000` improves versus the baseline

**Step 5: Stop threshold tuning when either exit condition is met**

Stop when:

- `threads=4, sort @100000 <= 1.35x`, or
- three consecutive threshold-only attempts fail to beat the best 4-thread result

**Step 6: Commit**

```bash
git add flow-algorithms/src/find_scan_sort.rs benchmarks/reports/taskflow_compare/sort_closeout_tune_threads1 benchmarks/reports/taskflow_compare/sort_closeout_tune_threads4
git commit -m "perf: tune sort thresholds against fixed benchmark contract"
```

## Task 4: Replace The Leaf Fallback Only If Threshold Tuning Plateaus

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`
- Verify: `flow-core/tests/runtimes.rs`

**Step 1: Remove the standard-library leaf dependency behind a single helper boundary**

Today the leaf path still ends here:

```rust
fn sort_slice_by_less<T, F>(slice: &mut [T], is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    if slice.len() < SORT_INSERTION_SORT_THRESHOLD {
        insertion_sort_by_less(slice, is_less);
    } else {
        slice.sort_unstable_by(|left, right| ordering_from_is_less(is_less, left, right));
    }
}
```

Replace that helper with a local sequential quicksort/pdqsort-style loop that reuses the same pivot/partition/fallback building blocks already used by `parallel_quicksort`.

**Step 2: Keep the scope narrow**

Do not add:

- SIMD fast paths
- radix sort
- type-specific special casing
- a new public option

**Step 3: Re-run correctness**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test runtimes -- --nocapture
```

Expected: PASS

**Step 4: Re-run canonical sort benchmarks**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 1 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_leaf_threads1
```

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_leaf_threads4
```

Expected:

- 1-thread result is at least neutral
- 4-thread `sort @100000` improves measurably over the best threshold-only result

**Step 5: Commit**

```bash
git add flow-algorithms/src/find_scan_sort.rs benchmarks/reports/taskflow_compare/sort_closeout_leaf_threads1 benchmarks/reports/taskflow_compare/sort_closeout_leaf_threads4
git commit -m "perf: localize sort leaf path after threshold plateau"
```

## Task 5: Run The Nearby Regression Subset

**Files:**
- Create: `benchmarks/reports/taskflow_compare/sort_closeout_regression_subset/taskflow_vs_rustflow_report.md`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`
- Verify: `flow-core/tests/runtimes.rs`

**Step 1: Run the focused test files again on the best sort candidate**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

Run:

```bash
cargo test -p flow-core --test runtimes -- --nocapture
```

Expected: PASS

**Step 2: Run the regression subset**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases scan,sort,merge_sort --output-dir benchmarks/reports/taskflow_compare/sort_closeout_regression_subset
```

Expected:

- `scan` stays near parity
- `sort` holds the improved result band
- `merge_sort` does not regress unexpectedly

**Step 3: Commit**

```bash
git add benchmarks/reports/taskflow_compare/sort_closeout_regression_subset
git commit -m "test: run sort closeout regression subset"
```

## Task 6: Update Project Status Docs

**Files:**
- Modify: `docs/plans/2026-04-05-post-lockfree-progress-report.md`
- Modify: `README.md`

**Step 1: Update the post-lockfree progress report**

Add:

- the frozen sort benchmark contract
- canonical baseline paths
- before/after `sort @100000` numbers
- whether the final improvement came from threshold tuning only or from the leaf rewrite
- the remaining gap, if any

**Step 2: Update README only if user-facing status changed materially**

Keep the README short. Only add one concise note if the sort gap meaningfully narrowed or if the benchmark methodology changed in a user-visible way.

**Step 3: Commit**

```bash
git add docs/plans/2026-04-05-post-lockfree-progress-report.md README.md
git commit -m "docs: record sort closeout results"
```

## Defer Pipeline Until After This Plan

Do not start new `pipeline` structural optimization during this plan unless:

- the sort benchmark contract is frozen
- the best sort candidate is benchmarked and documented
- the regression subset is green

At that point, start a separate pipeline-focused plan instead of extending this one.

## Exit Criteria

- The sort benchmark contract is explicit and stable in `benchmarks/src/bin/taskflow_compare.rs`.
- There is one canonical 1-thread and one canonical 4-thread sort baseline report for this closeout.
- `cargo test -p flow-algorithms --test find_scan_sort -- --nocapture` passes on the chosen candidate.
- `cargo test -p flow-core --test runtimes -- --nocapture` passes on the chosen candidate.
- `sort @100000` is improved and documented against the frozen contract.
- The post-lockfree progress report reflects the final sort status before pipeline work resumes.
