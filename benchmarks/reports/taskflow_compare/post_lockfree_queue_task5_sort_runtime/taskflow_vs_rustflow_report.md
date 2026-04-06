# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.011 | 0.016 | - |
| 100 | 0.001 | 0.010 | 0.081 | - |
| 1000 | 0.009 | 0.033 | 0.275 | - |
| 10000 | 0.115 | 0.125 | 0.920 | - |
| 100000 | 1.253 | 0.801 | 1.564 | - |

