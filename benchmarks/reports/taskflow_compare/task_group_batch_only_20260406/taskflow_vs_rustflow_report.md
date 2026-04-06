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
| 1 | 0.015 | 0.012 | 1.214 | - |
| 2 | 0.010 | 0.009 | 1.154 | - |
| 3 | 0.007 | 0.011 | 0.674 | - |
| 4 | 0.008 | 0.013 | 0.573 | - |
| 5 | 0.006 | 0.008 | 0.707 | - |
| 6 | 0.008 | 0.009 | 0.805 | - |
| 7 | 0.010 | 0.010 | 1.065 | - |
| 8 | 0.010 | 0.026 | 0.388 | - |
| 9 | 0.014 | 0.015 | 0.924 | - |
| 10 | 0.014 | 0.037 | 0.365 | - |
| 11 | 0.018 | 0.110 | 0.161 | - |
| 12 | 0.023 | 0.099 | 0.236 | - |
| 13 | 0.057 | 0.165 | 0.348 | - |
| 14 | 0.100 | 0.213 | 0.469 | - |
| 15 | 0.150 | 0.144 | 1.036 | - |
| 16 | 0.245 | 0.127 | 1.931 | - |
| 17 | 0.347 | 0.160 | 2.173 | - |
| 18 | 0.502 | 0.240 | 2.088 | - |
| 19 | 0.805 | 0.387 | 2.078 | - |
| 20 | 1.189 | 0.509 | 2.335 | - |

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.008 | 0.006 | 1.215 | - |
| 2 | 0.012 | 0.019 | 0.613 | - |
| 3 | 0.007 | 0.010 | 0.736 | - |
| 4 | 0.007 | 0.027 | 0.272 | - |
| 5 | 0.009 | 0.039 | 0.229 | - |
| 6 | 0.022 | 0.109 | 0.204 | - |
| 7 | 0.064 | 0.244 | 0.262 | - |
| 8 | 0.290 | 0.522 | 0.555 | - |
| 9 | 1.616 | 1.370 | 1.179 | - |
| 10 | 7.842 | 5.219 | 1.503 | - |

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.010 | 0.008 | 1.140 | - |
| 2 | 0.005 | 0.044 | 0.116 | - |
| 3 | 0.007 | 0.233 | 0.029 | - |
| 4 | 0.029 | 0.172 | 0.170 | - |

