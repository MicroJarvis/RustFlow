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
| 10 | 0.000 | 0.005 | 0.018 | - |
| 100 | 0.001 | 0.005 | 0.147 | - |
| 1000 | 0.007 | 0.023 | 0.294 | - |
| 10000 | 0.081 | 0.225 | 0.362 | - |
| 100000 | 0.935 | 2.098 | 0.446 | - |

