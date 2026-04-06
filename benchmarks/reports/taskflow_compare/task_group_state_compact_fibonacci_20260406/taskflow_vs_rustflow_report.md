# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Fibonacci

- Benchmark id: `fibonacci`
- Note: Report profile caps the original Taskflow sweep at `fib(20)` (original default: `fib(40)`).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.015 | 0.008 | 1.858 | - |
| 2 | 0.009 | 0.005 | 1.638 | - |
| 3 | 0.009 | 0.007 | 1.316 | - |
| 4 | 0.009 | 0.006 | 1.346 | - |
| 5 | 0.010 | 0.005 | 2.086 | - |
| 6 | 0.014 | 0.007 | 1.873 | - |
| 7 | 0.013 | 0.008 | 1.668 | - |
| 8 | 0.013 | 0.008 | 1.641 | - |
| 9 | 0.017 | 0.013 | 1.294 | - |
| 10 | 0.020 | 0.020 | 1.036 | - |
| 11 | 0.027 | 0.027 | 1.030 | - |
| 12 | 0.040 | 0.070 | 0.566 | - |
| 13 | 0.058 | 0.107 | 0.546 | - |
| 14 | 0.089 | 0.058 | 1.517 | - |
| 15 | 0.140 | 0.047 | 2.975 | - |
| 16 | 0.215 | 0.087 | 2.464 | - |
| 17 | 0.320 | 0.108 | 2.963 | - |
| 18 | 0.459 | 0.146 | 3.134 | - |
| 19 | 0.742 | 0.198 | 3.749 | - |
| 20 | 1.126 | 0.318 | 3.536 | - |

