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
| 2 | 0.035 | 0.038 | 0.918 | - |
| 4 | 0.053 | 0.027 | 1.952 | - |
| 8 | 0.100 | 0.036 | 2.779 | - |
| 16 | 0.196 | 0.042 | 4.709 | - |
| 32 | 0.382 | 0.070 | 5.453 | - |
| 64 | 0.749 | 0.119 | 6.291 | - |
| 128 | 1.465 | 0.269 | 5.453 | - |
| 256 | 2.966 | 0.475 | 6.243 | - |
| 512 | 5.698 | 0.933 | 6.107 | - |
| 1024 | 11.753 | 1.759 | 6.681 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.031 | 0.028 | 1.096 | - |
| 911 | 4.089 | 1.102 | 3.711 | - |
| 3334 | 15.612 | 3.487 | 4.478 | - |
| 7311 | 34.825 | 7.255 | 4.800 | - |
| 12904 | 60.886 | 12.813 | 4.752 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.227 | 0.160 | 1.418 | - |
| 4 | 0.535 | 0.178 | 3.006 | - |
| 8 | 0.977 | 0.324 | 3.016 | - |
| 16 | 1.908 | 0.548 | 3.480 | - |
| 32 | 3.799 | 1.033 | 3.679 | - |
| 64 | 7.593 | 2.015 | 3.768 | - |
| 128 | 15.168 | 3.964 | 3.827 | - |
| 256 | 30.435 | 7.741 | 3.931 | - |
| 512 | 60.663 | 15.620 | 3.884 | - |
| 1024 | 121.526 | 30.935 | 3.928 | - |

