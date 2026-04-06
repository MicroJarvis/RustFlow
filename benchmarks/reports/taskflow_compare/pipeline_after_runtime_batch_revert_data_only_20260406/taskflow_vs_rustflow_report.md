# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.036 | 0.125 | 0.291 | - |
| 4 | 0.056 | 0.063 | 0.895 | - |
| 8 | 0.126 | 0.093 | 1.354 | - |
| 16 | 0.330 | 0.139 | 2.377 | - |
| 32 | 0.582 | 0.337 | 1.729 | - |
| 64 | 1.133 | 0.525 | 2.158 | - |
| 128 | 2.089 | 0.777 | 2.690 | - |
| 256 | 3.754 | 1.088 | 3.449 | - |
| 512 | 6.559 | 2.000 | 3.279 | - |
| 1024 | 11.263 | 2.764 | 4.075 | - |

