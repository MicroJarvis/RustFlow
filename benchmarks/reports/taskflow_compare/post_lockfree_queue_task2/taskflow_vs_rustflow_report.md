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
| 1 | 0.021 | 0.009 | 2.307 | - |
| 2 | 0.010 | 0.007 | 1.297 | - |
| 3 | 0.011 | 0.007 | 1.599 | - |
| 4 | 0.014 | 0.006 | 2.281 | - |
| 5 | 0.016 | 0.007 | 2.144 | - |
| 6 | 0.014 | 0.007 | 1.980 | - |
| 7 | 0.014 | 0.007 | 2.025 | - |
| 8 | 0.018 | 0.010 | 1.851 | - |
| 9 | 0.026 | 0.009 | 2.927 | - |
| 10 | 0.033 | 0.013 | 2.533 | - |
| 11 | 0.046 | 0.019 | 2.477 | - |
| 12 | 0.070 | 0.022 | 3.190 | - |
| 13 | 0.109 | 0.033 | 3.278 | - |
| 14 | 0.172 | 0.067 | 2.584 | - |
| 15 | 0.273 | 0.104 | 2.615 | - |
| 16 | 0.409 | 0.086 | 4.759 | - |
| 17 | 0.622 | 0.147 | 4.219 | - |
| 18 | 0.953 | 0.262 | 3.639 | - |
| 19 | 1.545 | 0.222 | 6.971 | - |
| 20 | 2.484 | 4.146 | 0.599 | - |

## Integrate

- Benchmark id: `integrate`
- Note: Report profile caps the x-interval at 500 (original default: 2000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0.012 | 0.008 | 1.493 | - |
| 100 | 4.792 | 44.072 | 0.109 | - |
| 200 | 10.850 | 49.112 | 0.221 | - |
| 300 | 15.599 | 85.911 | 0.182 | - |
| 400 | 16.400 | 46.670 | 0.351 | - |
| 500 | 24.295 | 115.012 | 0.211 | - |

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.012 | 0.059 | 0.200 | - |
| 2 | 0.007 | 0.028 | 0.271 | - |
| 3 | 0.008 | 0.015 | 0.518 | - |
| 4 | 0.009 | 0.022 | 0.408 | - |
| 5 | 0.013 | 0.026 | 0.513 | - |
| 6 | 0.028 | 0.073 | 0.379 | - |
| 7 | 0.110 | 0.146 | 0.757 | - |
| 8 | 0.515 | 0.383 | 1.345 | - |
| 9 | 2.720 | 6.162 | 0.441 | - |
| 10 | 13.174 | 27.332 | 0.482 | - |

