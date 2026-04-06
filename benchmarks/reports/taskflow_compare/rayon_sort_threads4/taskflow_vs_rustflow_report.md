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
| 10 | 0.000 | 0.010 | 0.016 | - |
| 100 | 0.001 | 0.008 | 0.086 | - |
| 1000 | 0.007 | 0.025 | 0.284 | - |
| 10000 | 0.085 | 0.093 | 0.915 | - |
| 100000 | 0.952 | 0.631 | 1.509 | - |

