# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 1
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.094 | 0.064 | 1.469 | - |
| 4 | 0.109 | 0.042 | 2.605 | - |
| 8 | 0.195 | 0.047 | 4.148 | - |
| 16 | 0.355 | 0.077 | 4.608 | - |
| 32 | 0.760 | 0.126 | 6.031 | - |
| 64 | 2.039 | 0.223 | 9.144 | - |
| 128 | 3.582 | 0.437 | 8.197 | - |
| 256 | 6.428 | 0.933 | 6.889 | - |
| 512 | 10.660 | 1.512 | 7.050 | - |
| 1024 | 17.165 | 2.909 | 5.901 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.048 | 0.033 | 1.447 | - |
| 911 | 4.166 | 1.247 | 3.341 | - |
| 3334 | 15.410 | 3.675 | 4.193 | - |
| 7311 | 32.568 | 7.402 | 4.400 | - |
| 12904 | 56.117 | 11.245 | 4.990 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.285 | 0.156 | 1.825 | - |
| 4 | 0.517 | 0.205 | 2.521 | - |
| 8 | 1.005 | 0.335 | 3.001 | - |
| 16 | 2.046 | 0.593 | 3.450 | - |
| 32 | 3.949 | 1.150 | 3.434 | - |
| 64 | 7.808 | 2.130 | 3.666 | - |
| 128 | 15.623 | 4.217 | 3.705 | - |
| 256 | 31.053 | 8.356 | 3.716 | - |
| 512 | 62.107 | 16.551 | 3.752 | - |
| 1024 | 124.220 | 33.110 | 3.752 | - |

