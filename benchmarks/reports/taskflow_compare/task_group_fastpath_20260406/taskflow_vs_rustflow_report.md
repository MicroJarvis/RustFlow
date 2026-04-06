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
| 1 | 0.007 | 0.008 | 0.937 | - |
| 2 | 0.007 | 0.008 | 0.822 | - |
| 3 | 0.006 | 0.008 | 0.830 | - |
| 4 | 0.005 | 0.008 | 0.615 | - |
| 5 | 0.007 | 0.008 | 0.895 | - |
| 6 | 0.008 | 0.009 | 0.820 | - |
| 7 | 0.008 | 0.010 | 0.789 | - |
| 8 | 0.008 | 0.014 | 0.561 | - |
| 9 | 0.021 | 0.016 | 1.322 | - |
| 10 | 0.026 | 0.026 | 0.970 | - |
| 11 | 0.035 | 0.045 | 0.786 | - |
| 12 | 0.047 | 0.078 | 0.610 | - |
| 13 | 0.071 | 0.100 | 0.716 | - |
| 14 | 0.113 | 0.029 | 3.895 | - |
| 15 | 0.175 | 0.063 | 2.782 | - |
| 16 | 0.272 | 0.058 | 4.657 | - |
| 17 | 0.417 | 0.086 | 4.825 | - |
| 18 | 0.654 | 0.142 | 4.607 | - |
| 19 | 1.040 | 0.197 | 5.288 | - |
| 20 | 1.275 | 0.204 | 6.258 | - |

## Integrate

- Benchmark id: `integrate`
- Note: Report profile caps the x-interval at 500 (original default: 2000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0.007 | 0.010 | 0.693 | - |
| 100 | 1.657 | 7.549 | 0.220 | - |
| 200 | 3.904 | 13.095 | 0.298 | - |
| 300 | 7.048 | 23.795 | 0.296 | - |
| 400 | 8.555 | 29.090 | 0.294 | - |
| 500 | 13.489 | 46.471 | 0.290 | - |

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.011 | 0.012 | 0.933 | - |
| 2 | 0.007 | 0.011 | 0.631 | - |
| 3 | 0.006 | 0.018 | 0.337 | - |
| 4 | 0.007 | 0.024 | 0.273 | - |
| 5 | 0.011 | 0.058 | 0.194 | - |
| 6 | 0.022 | 0.188 | 0.115 | - |
| 7 | 0.066 | 0.219 | 0.302 | - |
| 8 | 0.346 | 0.457 | 0.757 | - |
| 9 | 1.823 | 1.361 | 1.340 | - |
| 10 | 8.371 | 5.118 | 1.636 | - |

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.011 | 0.014 | 0.770 | - |
| 2 | 0.008 | 0.036 | 0.214 | - |
| 3 | 0.007 | 0.270 | 0.027 | - |
| 4 | 0.028 | 0.272 | 0.104 | - |

