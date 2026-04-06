# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.012 | 0.008 | 1.490 | - |
| 2 | 0.006 | 0.015 | 0.386 | - |
| 3 | 0.007 | 0.010 | 0.710 | - |
| 4 | 0.009 | 0.016 | 0.556 | - |
| 5 | 0.011 | 0.035 | 0.326 | - |
| 6 | 0.025 | 0.110 | 0.228 | - |
| 7 | 0.071 | 0.316 | 0.225 | - |
| 8 | 0.372 | 0.486 | 0.765 | - |
| 9 | 1.782 | 1.446 | 1.233 | - |
| 10 | 8.434 | 5.889 | 1.432 | - |

