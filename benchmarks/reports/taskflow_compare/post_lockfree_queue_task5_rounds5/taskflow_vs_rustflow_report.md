# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Scan

- Benchmark id: `scan`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.016 | 0.019 | - |
| 100 | 0.000 | 0.018 | 0.011 | - |
| 1000 | 0.001 | 0.019 | 0.059 | - |
| 10000 | 0.018 | 0.020 | 0.884 | - |
| 100000 | 0.112 | 0.102 | 1.096 | - |

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.010 | 0.010 | - |
| 100 | 0.001 | 0.011 | 0.064 | - |
| 1000 | 0.006 | 0.028 | 0.215 | - |
| 10000 | 0.071 | 0.093 | 0.769 | - |
| 100000 | 0.949 | 0.611 | 1.554 | - |

