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
| 2 | 0.065 | 0.039 | 1.658 | - |
| 4 | 0.100 | 0.029 | 3.485 | - |
| 8 | 0.180 | 0.044 | 4.085 | - |
| 16 | 0.331 | 0.061 | 5.461 | - |
| 32 | 0.637 | 0.107 | 5.935 | - |
| 64 | 1.213 | 0.192 | 6.319 | - |
| 128 | 2.213 | 0.371 | 5.960 | - |
| 256 | 3.894 | 0.680 | 5.727 | - |
| 512 | 6.695 | 1.573 | 4.257 | - |
| 1024 | 11.416 | 2.888 | 3.953 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.029 | 0.024 | 1.229 | - |
| 911 | 4.046 | 0.905 | 4.471 | - |
| 3334 | 15.313 | 2.665 | 5.747 | - |
| 7311 | 32.526 | 6.118 | 5.317 | - |
| 12904 | 56.421 | 10.346 | 5.454 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.263 | 0.156 | 1.688 | - |
| 4 | 0.495 | 0.202 | 2.453 | - |
| 8 | 0.964 | 0.331 | 2.915 | - |
| 16 | 1.930 | 0.588 | 3.280 | - |
| 32 | 3.802 | 1.104 | 3.444 | - |
| 64 | 7.640 | 2.145 | 3.562 | - |
| 128 | 15.226 | 4.195 | 3.629 | - |
| 256 | 30.530 | 8.303 | 3.677 | - |
| 512 | 60.878 | 16.503 | 3.689 | - |
| 1024 | 121.732 | 32.936 | 3.696 | - |

