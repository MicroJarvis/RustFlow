# RustFlow

RustFlow is a Rust-native task graph runtime modeled after the CPU-side core of C++ Taskflow.
The public API centers on `Flow`, while the workspace is split into a reusable runtime crate,
an algorithms crate, and a benchmark harness.

## Current Status

As of 2026-03-31, the project is functionally beyond the bootstrap stage and the planned
capabilities in `doc/epics.md` are implemented in code.

- Core graph and executor: `Flow`, `FlowBuilder`, `TaskHandle`, DOT export, repeated runs,
  work-stealing executor, `run`, `run_n`, `run_until`, `wait_for_all`
- Control flow: condition tasks, multi-condition tasks, feedback loops, nested subflows with
  join semantics
- Runtime and composition: `RuntimeCtx`, runtime `schedule`, `corun`, executor async tasks,
  dependent async tasks, composed flows
- Reliability and observability: cooperative cancellation, semaphores, observer hooks, trace
  export, graph inspection and validation
- Algorithms: `parallel_for`, `reduce`, `transform`, `find`, inclusive and exclusive scan,
  sort
- Phase 2 additions: `Pipeline`, `ScalablePipeline`, `DataPipeline`, `compat` facade
- Benchmarks: runnable harness for `flow`, `threadpool`, `rayon`, and C++ `taskflow`

## Verification

- `cargo test --workspace` passes
- The benchmark harness runs from `cargo run -p benchmarks -- ...`
- The C++ Taskflow benchmark driver is compiled on demand under `target/benchmark-drivers/`

Example benchmark command:

```bash
cargo run -p benchmarks -- \
  --backends flow,threadpool,rayon,cpp-taskflow \
  --scenarios reduce \
  --input-len 65536 \
  --workers 8 \
  --chunk-size 1024 \
  --warmups 1 \
  --iterations 5 \
  --format table
```

## Workspace Layout

- `flow-core`
  - core graph model, executor, runtime, cancellation, observability, pipelines, compat facade
- `flow-algorithms`
  - parallel algorithms implemented on top of `flow-core`
- `benchmarks`
  - benchmark harness and C++ Taskflow comparison driver
- `docs`
  - development plan, epics, stories, and current performance work plan
- `taskflow`
  - local reference checkout of the upstream C++ project, not part of the Rust workspace

## Documents

- `docs/development-plan.md`
- `docs/epics.md`
- `docs/stories.md`
- `docs/plans/2026-04-05-post-lockfree-queue-performance-plan.md`
- `docs/plans/2026-04-05-post-lockfree-progress-report.md`
- `benchmarks/reports/taskflow_compare/current_live_20260405/taskflow_vs_rustflow_report.md`

## Suggested Next Focus

The remaining work is mostly engineering cleanup rather than missing core features:

- add CI to keep `cargo test --workspace` and benchmark smoke tests green
- publish stable benchmark baselines instead of only runnable harness code
- improve root-level documentation and examples for external users
- prepare crate metadata and release hygiene if the workspace will be published
