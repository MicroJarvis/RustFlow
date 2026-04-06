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
| 1 | 0.023 | 0.055 | 0.408 | - |
| 2 | 0.020 | 0.018 | 1.092 | - |
| 3 | 0.019 | 0.017 | 1.115 | - |
| 4 | 0.028 | 0.026 | 1.069 | - |
| 5 | 0.014 | 0.033 | 0.421 | - |
| 6 | 0.017 | 0.016 | 1.039 | - |
| 7 | 0.022 | 0.019 | 1.195 | - |
| 8 | 0.027 | 0.022 | 1.245 | - |
| 9 | 0.032 | 0.022 | 1.461 | - |
| 10 | 0.031 | 0.053 | 0.576 | - |
| 11 | 0.038 | 0.089 | 0.430 | - |
| 12 | 0.067 | 0.085 | 0.787 | - |
| 13 | 0.073 | 0.165 | 0.443 | - |
| 14 | 0.129 | 0.136 | 0.950 | - |
| 15 | 0.192 | 0.522 | 0.368 | - |
| 16 | 0.290 | 0.303 | 0.957 | - |
| 17 | 0.413 | 0.317 | 1.303 | - |
| 18 | 0.624 | 0.335 | 1.862 | - |
| 19 | 0.881 | 0.401 | 2.199 | - |
| 20 | 1.313 | 0.436 | 3.010 | - |

