# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.056 | 0.038 | 1.469 | - |
| 4 | 0.089 | 0.034 | 2.612 | - |
| 8 | 0.172 | 0.050 | 3.430 | - |
| 16 | 0.293 | 0.063 | 4.623 | - |
| 32 | 0.571 | 0.103 | 5.538 | - |
| 64 | 1.026 | 0.189 | 5.433 | - |
| 128 | 1.909 | 0.362 | 5.273 | - |
| 256 | 3.359 | 0.707 | 4.752 | - |
| 512 | 5.857 | 1.473 | 3.976 | - |
| 1024 | 10.553 | 2.259 | 4.671 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.303 | 0.180 | 1.682 | - |
| 4 | 0.575 | 0.236 | 2.434 | - |
| 8 | 1.118 | 0.384 | 2.915 | - |
| 16 | 2.232 | 0.698 | 3.199 | - |
| 32 | 4.431 | 1.301 | 3.405 | - |
| 64 | 8.812 | 2.495 | 3.531 | - |
| 128 | 17.528 | 4.897 | 3.579 | - |
| 256 | 36.314 | 9.697 | 3.745 | - |
| 512 | 73.429 | 19.198 | 3.825 | - |
| 1024 | 142.572 | 38.128 | 3.739 | - |

