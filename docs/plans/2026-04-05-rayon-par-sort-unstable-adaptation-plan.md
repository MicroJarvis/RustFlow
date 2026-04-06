# Rayon-Style Parallel Sort Adaptation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace RustFlow's current `select_nth_unstable_by`-driven parallel sort core with a RustFlow-owned parallel pdqsort-style implementation modeled on `rayon::par_sort_unstable`, and close the remaining `sort @100000` benchmark gap without changing the public API.

**Architecture:** Keep `parallel_sort` and `parallel_sort_by` public signatures unchanged in `flow-algorithms`. Replace the internal recursive sorter in `find_scan_sort.rs` with a Rayon-style structure: sequential partitioning plus parallel recursion using `RuntimeCtx::silent_async`, tail-recursing on one side and spawning the other. Borrow the algorithmic structure from Rayon and `core::slice::sort_unstable`, but keep the implementation local to RustFlow and avoid adding a Rayon dependency.

**Tech Stack:** Rust stable, `flow-core` runtime children, `flow-algorithms`, workspace tests, `benchmarks` `taskflow_compare` harness, local Taskflow checkout, Rayon source/docs as algorithm reference.

---

## Context

This plan is based on:

- `flow-algorithms/src/find_scan_sort.rs`
- `flow-algorithms/tests/find_scan_sort.rs`
- `benchmarks/src/bin/taskflow_compare.rs`
- `benchmarks/reports/taskflow_compare/post_lockfree_queue_task5_rounds5/taskflow_vs_rustflow_report.md`
- `benchmarks/reports/taskflow_compare/post_lockfree_queue_task5_sort_cutoff32k/taskflow_vs_rustflow_report.md`
- Rayon docs for `ParallelSliceMut::par_sort_unstable`
- Rayon source `rayon/slice/sort.rs`

Current observations:

1. The current RustFlow sort still centers each recursive step on `slice.select_nth_unstable_by(mid, ...)`.
2. That path gets acceptable sequential performance, but it still trails Taskflow materially at `sort @100000`.
3. The remaining gap looks algorithmic, not only scheduler-related.
4. Rayon already exposes a mature parallel pdqsort-style layout on stable Rust:
   - partitioning is sequential
   - recursive halves are parallel
   - small partitions use insertion-sort-style paths
   - bad partitions fall back to heapsort
   - pattern handling avoids common quicksort degeneration

## Guardrails

- Do not change the public `parallel_sort` / `parallel_sort_by` API.
- Do not add a new third-party dependency just to reuse the algorithm.
- Keep the sort in-place and unstable.
- Preserve panic safety and comparator semantics.
- Keep `RuntimeCtx`-based recursion; do not regress the current runtime-child fast path.
- Prefer landing this in small reviewable steps instead of a one-shot large port.

## Non-Goals

- Do not attempt a byte-for-byte port of Rayon.
- Do not optimize `scan`, `find`, or `pipeline` in this plan.
- Do not add special-case SIMD or radix sort paths in the first pass.
- Do not introduce type-specific fast paths before the generic path is correct and benchmarked.

## Status As Of 2026-04-05

Completed in commit `00c9303` (`refactor: prepare parallel sort for rayon-style rewrite`):

- Task 1: expanded `parallel_sort` / `parallel_sort_by` correctness coverage in `flow-algorithms/tests/find_scan_sort.rs`
- Task 2: refactored the internal sort entry to use a Rayon-style `is_less` comparator path and dedicated runtime sort entry points
- Task 3: added insertion-sort, partial-insertion-sort, heapsort, and bad-partition-budget scaffolding in `flow-algorithms/src/find_scan_sort.rs`

Verification already completed for the landed batch:

- `cargo test -p flow-algorithms --test find_scan_sort -- --nocapture`

Important current state:

- The sort execution skeleton is now prepared for a Rayon-style rewrite.
- The main recursive partitioning path still uses `select_nth_unstable_by`.
- The new helper/fallback building blocks are present but not yet wired into the main partition logic.

## Completed Task 1: Lock Down Correctness Coverage For Sort Shapes

**Files:**
- Modify: `flow-algorithms/tests/find_scan_sort.rs`
- Verify: `flow-algorithms/src/find_scan_sort.rs`

Completed work:

- added large-input tests covering:
  - already sorted input
  - reverse-sorted input
  - duplicate-heavy input
  - all-equal input
  - nearly sorted input
- added `parallel_sort_by` descending-order oracle tests
- used both `sort_unstable` and Rayon `par_sort_unstable` / `par_sort_unstable_by` as correctness oracles
- kept Rayon restricted to `dev-dependencies`

Verification run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

## Completed Task 2: Introduce A Rayon-Style Internal Sort Skeleton

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`

Completed work:

- introduced a local `is_less` comparator adapter for the internal sort path
- split the top-level entry into a dedicated runtime sort entry plus internal quicksort-style loop
- preserved the runtime-child execution shape:
  - spawn one side with `RuntimeCtx::silent_async`
  - iterate on the other side
  - call `runtime.corun_children()` once at the end
- kept the public `parallel_sort` / `parallel_sort_by` API unchanged

Verification run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

## Completed Task 3: Port The Small-Partition Building Blocks

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`

Completed work:

- added insertion sort helpers for short partitions
- added bounded partial insertion sort scaffolding for nearly sorted partitions
- added heapsort fallback helper and bad-partition budget initialization
- left these helpers staged for integration into the main partitioning path in the next task

Verification run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

## Remaining Tasks

## Task 4: Replace `select_nth_unstable_by` With Explicit Pivot + Partition

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`

**Step 1: Add pivot selection helpers**

Implement the minimal pivot-selection set inspired by Rayon:

- median-of-three
- Tukey ninther for larger slices

Do not over-generalize this API.

**Step 2: Add explicit partition routine**

Replace the `slice.select_nth_unstable_by(mid, ...)` call with an explicit partition function that returns:

- pivot position
- whether the data looked already partitioned

First pass can be the non-branchless version. Leave branchless partition as an optional follow-up.

**Step 3: Add pattern-defeating handling**

Handle the high-value cases:

- equal-element-heavy partitions
- highly unbalanced partitions
- nearly sorted partitions

At minimum:

- decrement a bad-partition budget
- switch to heapsort when budget is exhausted
- try `partial_insertion_sort` when partitioning reports a favorable shape

**Step 4: Preserve tail-recursive runtime structure**

After partitioning:

- spawn one side
- continue on the other side in the current function
- avoid reintroducing per-level `TaskGroup` joins

**Step 5: Run tests**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

**Step 6: Commit**

```bash
git add flow-algorithms/src/find_scan_sort.rs flow-algorithms/tests/find_scan_sort.rs
git commit -m "feat: replace nth-element recursion with pdqsort-style partitioning"
```

## Task 5: Tune Parallel Thresholds Against The New Core

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `benchmarks/src/bin/taskflow_compare.rs`
- Benchmark: `benchmarks/reports/taskflow_compare/*`

**Step 1: Revisit threshold constants**

Tune, at minimum:

- parallel entry cutoff
- insertion-sort threshold
- ninther threshold
- bad-partition budget initialization

Keep constants local and documented.

**Step 2: Run targeted correctness checks after each threshold change**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS after every tuning round

**Step 3: Run targeted sort benchmarks**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 1 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/rayon_sort_threads1
```

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 5 --cases sort --output-dir benchmarks/reports/taskflow_compare/rayon_sort_threads4
```

Expected:

- 1-thread result should not regress materially from current sequential behavior
- 4-thread `sort @100000` should improve versus the current `~1.54x` band

**Step 4: Commit**

```bash
git add flow-algorithms/src/find_scan_sort.rs benchmarks/reports/taskflow_compare/rayon_sort_threads1 benchmarks/reports/taskflow_compare/rayon_sort_threads4
git commit -m "perf: tune rayon-style parallel sort thresholds"
```

## Task 6: Run Broader Regression Checks

**Files:**
- Verify: `flow-algorithms/tests/find_scan_sort.rs`
- Verify: `flow-core/tests/runtimes.rs`
- Verify: `benchmarks/src/bin/taskflow_compare.rs`

**Step 1: Run the algorithm test file**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

**Step 2: Run runtime tests**

Run:

```bash
cargo test -p flow-core --test runtimes -- --nocapture
```

Expected: PASS

**Step 3: Run nearby benchmark cases to catch regressions**

Run:

```bash
cargo run -p benchmarks --release --bin taskflow_compare -- --threads 4 --rounds 3 --cases scan,sort,merge_sort --output-dir benchmarks/reports/taskflow_compare/rayon_sort_regression_subset
```

Expected:

- `scan` remains near parity
- `sort` improves
- `merge_sort` does not unexpectedly regress due to benchmark-shape overlap

**Step 4: Commit**

```bash
git add benchmarks/reports/taskflow_compare/rayon_sort_regression_subset
git commit -m "test: verify rayon-style sort against neighboring benchmark cases"
```

## Task 7: Document The Outcome

**Files:**
- Modify: `docs/plans/2026-04-05-post-lockfree-progress-report.md`
- Optionally modify: `README.md`

**Step 1: Update the progress report**

Add:

- what was borrowed from Rayon
- what was intentionally not ported
- before/after benchmark numbers for `sort`
- any remaining gaps

**Step 2: Update README only if the user-facing status materially changed**

Do not add churn if the change is purely internal.

**Step 3: Commit**

```bash
git add docs/plans/2026-04-05-post-lockfree-progress-report.md README.md
git commit -m "docs: record rayon-style parallel sort adaptation results"
```

## Recommended Execution Order

1. Task 4: replace `select_nth_unstable_by` with explicit pivot selection and partitioning.
2. Task 5: tune thresholds only after Task 4 lands and tests are green.
3. Task 6: run broader regression checks and capture targeted benchmark results.
4. Task 7: update progress docs with before/after benchmark numbers and remaining gaps.

## Exit Criteria

- `parallel_sort` no longer depends on `select_nth_unstable_by` for its main recursive partitioning path.
- The implementation structure matches Rayon at a high level:
  - sequential partition
  - parallel recursive halves
  - insertion-sort path for tiny slices
  - heapsort fallback for degenerate shapes
- `cargo test -p flow-algorithms --test find_scan_sort -- --nocapture` passes.
- `cargo test -p flow-core --test runtimes -- --nocapture` passes.
- `sort @100000` improves materially from the current `~1.54x` ratio band.
