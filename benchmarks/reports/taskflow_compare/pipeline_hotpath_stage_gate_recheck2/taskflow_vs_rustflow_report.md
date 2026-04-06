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
| 2 | 0.053 | 0.041 | 1.310 | - |
| 4 | 0.081 | 0.035 | 2.300 | - |
| 8 | 0.153 | 0.045 | 3.385 | - |
| 16 | 0.293 | 0.076 | 3.852 | - |
| 32 | 0.569 | 0.127 | 4.478 | - |
| 64 | 1.099 | 0.224 | 4.905 | - |
| 128 | 2.025 | 0.375 | 5.401 | - |
| 256 | 3.661 | 0.837 | 4.374 | - |
| 512 | 6.401 | 1.903 | 3.364 | - |
| 1024 | 11.138 | 2.597 | 4.289 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.034 | 0.023 | 1.483 | - |
| 911 | 3.960 | 0.794 | 4.990 | - |
| 3334 | 14.701 | 2.928 | 5.021 | - |
| 7311 | 32.252 | 5.911 | 5.457 | - |
| 12904 | 56.042 | 10.236 | 5.475 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.269 | 0.158 | 1.705 | - |
| 4 | 0.501 | 0.184 | 2.717 | - |
| 8 | 0.967 | 0.335 | 2.889 | - |
| 16 | 1.911 | 0.591 | 3.231 | - |
| 32 | 3.822 | 1.106 | 3.455 | - |
| 64 | 7.628 | 2.155 | 3.539 | - |
| 128 | 15.254 | 4.193 | 3.638 | - |
| 256 | 30.463 | 8.296 | 3.672 | - |
| 512 | 61.071 | 16.524 | 3.696 | - |
| 1024 | 121.995 | 32.983 | 3.699 | - |

