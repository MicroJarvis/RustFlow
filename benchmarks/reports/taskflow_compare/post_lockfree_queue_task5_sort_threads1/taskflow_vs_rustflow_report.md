# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 1
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.006 | 0.021 | - |
| 100 | 0.001 | 0.007 | 0.090 | - |
| 1000 | 0.006 | 0.020 | 0.313 | - |
| 10000 | 0.088 | 0.186 | 0.475 | - |
| 100000 | 0.966 | 1.843 | 0.524 | - |

