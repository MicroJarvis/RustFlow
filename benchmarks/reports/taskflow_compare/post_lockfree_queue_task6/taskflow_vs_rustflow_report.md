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
| 2 | 0.041 | 0.048 | 0.846 | - |
| 4 | 0.056 | 0.030 | 1.874 | - |
| 8 | 0.103 | 0.038 | 2.707 | - |
| 16 | 0.197 | 0.060 | 3.310 | - |
| 32 | 0.383 | 0.107 | 3.572 | - |
| 64 | 0.774 | 0.175 | 4.430 | - |
| 128 | 1.487 | 0.281 | 5.287 | - |
| 256 | 3.020 | 0.572 | 5.284 | - |
| 512 | 6.057 | 1.091 | 5.552 | - |
| 1024 | 12.046 | 2.025 | 5.948 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.027 | 0.068 | 0.396 | - |
| 911 | 4.231 | 1.231 | 3.437 | - |
| 3334 | 15.438 | 4.401 | 3.508 | - |
| 7311 | 34.079 | 8.424 | 4.045 | - |
| 12904 | 59.934 | 14.587 | 4.109 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.289 | 0.215 | 1.342 | - |
| 4 | 0.581 | 0.246 | 2.364 | - |
| 8 | 1.182 | 0.387 | 3.053 | - |
| 16 | 2.345 | 0.650 | 3.610 | - |
| 32 | 4.547 | 1.187 | 3.830 | - |
| 64 | 9.176 | 2.346 | 3.911 | - |
| 128 | 18.148 | 4.884 | 3.715 | - |
| 256 | 36.144 | 9.537 | 3.790 | - |
| 512 | 73.496 | 18.660 | 3.939 | - |
| 1024 | 145.006 | 37.756 | 3.841 | - |

