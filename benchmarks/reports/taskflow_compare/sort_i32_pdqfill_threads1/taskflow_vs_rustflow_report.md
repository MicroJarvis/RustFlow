# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 1
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8). Taskflow driver is adapted to i32 input for apples-to-apples comparison.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.009 | 0.015 | - |
| 100 | 0.001 | 0.004 | 0.215 | - |
| 1000 | 0.009 | 0.015 | 0.597 | - |
| 10000 | 0.107 | 0.135 | 0.791 | - |
| 100000 | 1.213 | 2.611 | 0.465 | - |

