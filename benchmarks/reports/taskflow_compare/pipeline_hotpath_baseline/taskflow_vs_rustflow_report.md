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
| 2 | 0.062 | 0.040 | 1.558 | - |
| 4 | 0.092 | 0.038 | 2.409 | - |
| 8 | 0.166 | 0.048 | 3.445 | - |
| 16 | 0.323 | 0.063 | 5.105 | - |
| 32 | 0.635 | 0.107 | 5.955 | - |
| 64 | 1.173 | 0.202 | 5.799 | - |
| 128 | 2.126 | 0.367 | 5.799 | - |
| 256 | 3.766 | 0.403 | 9.353 | - |
| 512 | 6.580 | 0.953 | 6.902 | - |
| 1024 | 11.287 | 1.675 | 6.737 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.032 | 0.023 | 1.355 | - |
| 911 | 3.809 | 0.804 | 4.738 | - |
| 3334 | 14.337 | 2.612 | 5.489 | - |
| 7311 | 30.903 | 5.684 | 5.436 | - |
| 12904 | 54.231 | 11.042 | 4.911 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.267 | 0.155 | 1.729 | - |
| 4 | 0.491 | 0.200 | 2.457 | - |
| 8 | 0.978 | 0.331 | 2.952 | - |
| 16 | 1.914 | 0.590 | 3.246 | - |
| 32 | 3.832 | 1.114 | 3.440 | - |
| 64 | 7.596 | 2.139 | 3.551 | - |
| 128 | 15.303 | 4.170 | 3.670 | - |
| 256 | 30.546 | 8.298 | 3.681 | - |
| 512 | 61.035 | 16.582 | 3.681 | - |
| 1024 | 122.501 | 33.058 | 3.706 | - |

