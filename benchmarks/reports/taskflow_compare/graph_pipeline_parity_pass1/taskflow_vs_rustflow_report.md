# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.035 | 0.038 | 0.924 | - |
| 911 | 3.912 | 1.581 | 2.474 | - |
| 3334 | 14.754 | 4.591 | 3.214 | - |
| 7311 | 31.931 | 7.330 | 4.356 | - |
| 12904 | 56.062 | 11.574 | 4.844 | - |

