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
| 2 | 0.031 | 0.027 | 1.167 | - |
| 911 | 4.048 | 1.023 | 3.955 | - |
| 3334 | 14.415 | 3.206 | 4.497 | - |
| 7311 | 31.535 | 6.258 | 5.039 | - |
| 12904 | 55.400 | 11.344 | 4.883 | - |

