# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Primes

- Benchmark id: `primes`
- Note: Report profile caps the original Taskflow sweep at 10^5 (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.039 | 0.569 | 0.069 | - |
| 100 | 0.016 | 0.132 | 0.125 | - |
| 1000 | 0.017 | 0.013 | 1.310 | - |
| 10000 | 0.083 | 0.040 | 2.089 | - |
| 100000 | 0.752 | 0.361 | 2.086 | - |

## Reduce Sum

- Benchmark id: `reduce_sum`
- Note: Report profile caps the original Taskflow sweep at 10^6 elements (original default: 10^9).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.070 | 0.002 | - |
| 100 | 0.000 | 0.010 | 0.010 | - |
| 1000 | 0.001 | 0.009 | 0.101 | - |
| 10000 | 0.010 | 0.014 | 0.730 | - |
| 100000 | 0.057 | 0.042 | 1.356 | - |
| 1000000 | 0.268 | 4.612 | 0.058 | - |

## Scan

- Benchmark id: `scan`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 2.719 | 0.000 | - |
| 100 | 0.000 | 0.166 | 0.001 | - |
| 1000 | 0.002 | 0.013 | 0.160 | - |
| 10000 | 0.041 | 0.019 | 2.121 | - |
| 100000 | 0.223 | 0.354 | 0.629 | - |

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.010 | 0.022 | - |
| 100 | 0.001 | 0.013 | 0.077 | - |
| 1000 | 0.012 | 0.021 | 0.583 | - |
| 10000 | 0.170 | 0.108 | 1.568 | - |
| 100000 | 2.161 | 2.017 | 1.071 | - |

