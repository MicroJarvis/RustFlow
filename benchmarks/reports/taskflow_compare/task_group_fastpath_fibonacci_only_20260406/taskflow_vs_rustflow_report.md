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
| 1 | 0.009 | 0.008 | 1.161 | - |
| 2 | 0.009 | 0.006 | 1.364 | - |
| 3 | 0.007 | 0.006 | 1.160 | - |
| 4 | 0.009 | 0.012 | 0.713 | - |
| 5 | 0.009 | 0.007 | 1.323 | - |
| 6 | 0.009 | 0.008 | 1.210 | - |
| 7 | 0.007 | 0.009 | 0.813 | - |
| 8 | 0.011 | 0.015 | 0.714 | - |
| 9 | 0.016 | 0.024 | 0.662 | - |
| 10 | 0.018 | 0.033 | 0.556 | - |
| 11 | 0.026 | 0.032 | 0.810 | - |
| 12 | 0.035 | 0.049 | 0.714 | - |
| 13 | 0.056 | 0.110 | 0.512 | - |
| 14 | 0.087 | 0.026 | 3.398 | - |
| 15 | 0.138 | 0.056 | 2.464 | - |
| 16 | 0.204 | 0.052 | 3.897 | - |
| 17 | 0.305 | 0.089 | 3.419 | - |
| 18 | 0.446 | 0.123 | 3.628 | - |
| 19 | 0.763 | 0.178 | 4.293 | - |
| 20 | 1.167 | 0.259 | 4.511 | - |

