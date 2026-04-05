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

## Task 1: Lock Down Correctness Coverage For Sort Shapes

**Files:**
- Modify: `flow-algorithms/tests/find_scan_sort.rs`
- Verify: `flow-algorithms/src/find_scan_sort.rs`

**Step 1: Add failing or currently-missing tests for difficult sort shapes**

Add tests covering:

- already sorted input
- reverse-sorted input
- heavily duplicated input
- all-equal input
- near-sorted input with a few local inversions

Use `parallel_sort` and `parallel_sort_by`, and compare with `sort_unstable` / `sort_unstable_by`.

**Step 2: Run the targeted test file**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected:

- PASS if coverage is additive only
- otherwise fail in a way that reveals current sort edge-case bugs before the algorithm rewrite

**Step 3: Commit**

```bash
git add flow-algorithms/tests/find_scan_sort.rs
git commit -m "test: expand parallel sort coverage for hard input shapes"
```

## Task 2: Introduce A Rayon-Style Internal Sort Skeleton

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`

**Step 1: Add an internal comparator adapter**

Introduce a local `is_less` style adapter so the internal sorter can work with `Fn(&T, &T) -> bool` rather than repeatedly comparing `Ordering::Less`.

Target shape:

```rust
let is_less = Arc::new(move |a: &T, b: &T| compare(a, b) == Ordering::Less);
```

Keep the public `parallel_sort_by` API unchanged.

**Step 2: Split the top-level sort into sequential gate plus internal parallel entry**

Refactor `parallel_sort_by` so it:

- computes worker count and sequential cutoff
- chooses the sequential path for tiny inputs
- otherwise enters a dedicated internal quicksort function

Do not port pdqsort behavior yet in this step; only make the call graph ready.

**Step 3: Keep runtime recursion as the execution shape**

Retain the current best structure:

- spawn one side with `RuntimeCtx::silent_async`
- iterate on the other side in a loop
- call `runtime.corun_children()` once at the end

**Step 4: Run the sort test file**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add flow-algorithms/src/find_scan_sort.rs flow-algorithms/tests/find_scan_sort.rs
git commit -m "refactor: prepare parallel sort for rayon-style pdqsort core"
```

## Task 3: Port The Small-Partition Building Blocks

**Files:**
- Modify: `flow-algorithms/src/find_scan_sort.rs`
- Verify: `flow-algorithms/tests/find_scan_sort.rs`

**Step 1: Add insertion-sort helpers**

Port or adapt the minimal helpers needed for small partitions:

- insertion of head/tail
- insertion sort for short slices

Keep them private to `find_scan_sort.rs`.

**Step 2: Add partial insertion sort**

Introduce a bounded `partial_insertion_sort` helper for nearly sorted partitions.

Expected use:

- when partitioning reports the slice was already almost partitioned
- as an early exit before deeper recursion

**Step 3: Add heapsort fallback helper**

Introduce a private heapsort fallback and a recursion-budget / bad-partition counter.

Goal:

- guarantee worst-case behavior
- match Rayon/`pdqsort` shape more closely than the current median-selection recursion

**Step 4: Run tests**

Run:

```bash
cargo test -p flow-algorithms --test find_scan_sort -- --nocapture
```

Expected: PASS

**Step 5: Commit**

```bash
git add flow-algorithms/src/find_scan_sort.rs flow-algorithms/tests/find_scan_sort.rs
git commit -m "feat: add pdqsort small-partition and fallback helpers"
```

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

1. Land Task 1 first so the rewrite has edge-case protection.
2. Land Task 2 before any algorithm port, so the internal API is clean.
3. Land Tasks 3 and 4 as separate commits if possible.
4. Treat Task 5 as benchmark-guided tuning, not open-ended experimentation.
5. Only move to pipeline work after Task 6 confirms the sort rewrite is a real win.

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

