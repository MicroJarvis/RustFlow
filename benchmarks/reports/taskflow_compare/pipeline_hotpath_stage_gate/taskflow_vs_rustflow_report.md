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
| 2 | 0.066 | 0.049 | 1.344 | - |
| 4 | 0.099 | 0.051 | 1.926 | - |
| 8 | 0.189 | 0.069 | 2.723 | - |
| 16 | 0.365 | 0.079 | 4.641 | - |
| 32 | 0.649 | 0.159 | 4.072 | - |
| 64 | 1.259 | 0.279 | 4.519 | - |
| 128 | 2.251 | 0.433 | 5.200 | - |
| 256 | 3.895 | 0.663 | 5.878 | - |
| 512 | 6.675 | 1.688 | 3.955 | - |
| 1024 | 11.430 | 2.588 | 4.416 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.034 | 0.023 | 1.497 | - |
| 911 | 3.749 | 0.885 | 4.234 | - |
| 3334 | 14.206 | 2.996 | 4.741 | - |
| 7311 | 30.765 | 6.418 | 4.793 | - |
| 12904 | 53.890 | 10.960 | 4.917 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.263 | 0.158 | 1.671 | - |
| 4 | 0.498 | 0.200 | 2.495 | - |
| 8 | 0.971 | 0.347 | 2.796 | - |
| 16 | 1.976 | 0.594 | 3.325 | - |
| 32 | 3.819 | 1.100 | 3.472 | - |
| 64 | 7.760 | 2.133 | 3.638 | - |
| 128 | 15.436 | 4.212 | 3.665 | - |
| 256 | 30.829 | 8.303 | 3.713 | - |
| 512 | 61.743 | 16.659 | 3.706 | - |
| 1024 | 123.666 | 33.148 | 3.731 | - |

