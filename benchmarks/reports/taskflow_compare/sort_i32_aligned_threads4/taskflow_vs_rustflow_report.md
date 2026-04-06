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
| 10 | 0.000 | 0.021 | 0.007 | - |
| 100 | 0.001 | 0.029 | 0.032 | - |
| 1000 | 0.009 | 0.037 | 0.245 | - |
| 10000 | 0.108 | 0.105 | 1.033 | - |
| 100000 | 1.158 | 0.708 | 1.635 | - |

