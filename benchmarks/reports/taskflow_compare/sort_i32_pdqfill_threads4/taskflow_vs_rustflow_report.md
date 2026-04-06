# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8). Taskflow driver is adapted to i32 input for apples-to-apples comparison.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.018 | 0.007 | - |
| 100 | 0.001 | 0.011 | 0.063 | - |
| 1000 | 0.007 | 0.024 | 0.294 | - |
| 10000 | 0.085 | 0.088 | 0.966 | - |
| 100000 | 0.940 | 0.572 | 1.642 | - |

