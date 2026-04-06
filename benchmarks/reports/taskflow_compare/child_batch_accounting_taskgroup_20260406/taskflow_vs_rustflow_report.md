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
| 1 | 0.026 | 0.012 | 2.163 | - |
| 2 | 0.017 | 0.053 | 0.314 | - |
| 3 | 0.019 | 0.009 | 2.061 | - |
| 4 | 0.016 | 0.015 | 1.021 | - |
| 5 | 0.017 | 0.019 | 0.881 | - |
| 6 | 0.020 | 0.017 | 1.221 | - |
| 7 | 0.016 | 0.025 | 0.647 | - |
| 8 | 0.021 | 0.082 | 0.257 | - |
| 9 | 0.036 | 0.022 | 1.665 | - |
| 10 | 0.031 | 0.035 | 0.882 | - |
| 11 | 0.036 | 0.073 | 0.493 | - |
| 12 | 0.059 | 0.090 | 0.651 | - |
| 13 | 0.078 | 0.087 | 0.897 | - |
| 14 | 0.143 | 0.178 | 0.802 | - |
| 15 | 0.177 | 0.239 | 0.740 | - |
| 16 | 0.148 | 0.364 | 0.407 | - |
| 17 | 0.348 | 0.251 | 1.389 | - |
| 18 | 0.622 | 0.212 | 2.928 | - |
| 19 | 0.843 | 0.334 | 2.525 | - |
| 20 | 1.316 | 0.434 | 3.029 | - |

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.007 | 0.010 | 0.733 | - |
| 2 | 0.006 | 0.010 | 0.597 | - |
| 3 | 0.007 | 0.010 | 0.657 | - |
| 4 | 0.008 | 0.020 | 0.397 | - |
| 5 | 0.009 | 0.044 | 0.211 | - |
| 6 | 0.018 | 0.157 | 0.116 | - |
| 7 | 0.061 | 0.208 | 0.291 | - |
| 8 | 0.318 | 0.361 | 0.881 | - |
| 9 | 1.672 | 1.218 | 1.373 | - |
| 10 | 7.979 | 4.958 | 1.609 | - |

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.006 | 0.008 | 0.712 | - |
| 2 | 0.005 | 0.031 | 0.148 | - |
| 3 | 0.007 | 0.163 | 0.046 | - |
| 4 | 0.022 | 0.195 | 0.110 | - |

