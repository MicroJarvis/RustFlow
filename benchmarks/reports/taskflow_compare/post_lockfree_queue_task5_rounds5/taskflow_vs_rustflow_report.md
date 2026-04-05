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
| 10 | 0.000 | 0.016 | 0.006 | - |
| 100 | 0.000 | 0.016 | 0.029 | - |
| 1000 | 0.001 | 0.014 | 0.082 | - |
| 10000 | 0.018 | 0.022 | 0.819 | - |
| 100000 | 0.110 | 0.103 | 1.072 | - |

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.022 | 0.007 | - |
| 100 | 0.001 | 0.013 | 0.050 | - |
| 1000 | 0.006 | 0.026 | 0.236 | - |
| 10000 | 0.084 | 0.103 | 0.818 | - |
| 100000 | 1.067 | 0.605 | 1.763 | - |

