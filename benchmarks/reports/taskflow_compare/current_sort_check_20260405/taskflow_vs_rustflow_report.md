# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8). Taskflow driver is adapted to i32 input for apples-to-apples comparison.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.023 | 0.009 | - |
| 100 | 0.001 | 0.008 | 0.120 | - |
| 1000 | 0.009 | 0.020 | 0.477 | - |
| 10000 | 0.108 | 0.069 | 1.567 | - |
| 100000 | 1.190 | 0.697 | 1.707 | - |

