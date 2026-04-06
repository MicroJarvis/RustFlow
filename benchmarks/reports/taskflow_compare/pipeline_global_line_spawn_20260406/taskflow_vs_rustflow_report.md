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
| 2 | 0.133 | 0.141 | 0.941 | - |
| 4 | 0.203 | 0.039 | 5.207 | - |
| 8 | 0.144 | 0.045 | 3.210 | - |
| 16 | 0.278 | 0.072 | 3.868 | - |
| 32 | 0.573 | 0.147 | 3.896 | - |
| 64 | 0.778 | 0.214 | 3.637 | - |
| 128 | 2.084 | 0.365 | 5.709 | - |
| 256 | 3.469 | 0.728 | 4.766 | - |
| 512 | 6.354 | 1.453 | 4.373 | - |
| 1024 | 10.716 | 3.311 | 3.236 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.046 | 0.039 | 1.189 | - |
| 911 | 2.498 | 1.358 | 1.839 | - |
| 3334 | 8.383 | 3.943 | 2.126 | - |
| 7311 | 17.694 | 7.848 | 2.255 | - |
| 12904 | 31.770 | 12.421 | 2.558 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.173 | 0.408 | 0.425 | - |
| 4 | 0.294 | 0.213 | 1.379 | - |
| 8 | 0.562 | 0.382 | 1.471 | - |
| 16 | 1.078 | 0.539 | 2.000 | - |
| 32 | 2.090 | 1.018 | 2.053 | - |
| 64 | 4.066 | 2.018 | 2.015 | - |
| 128 | 8.104 | 4.014 | 2.019 | - |
| 256 | 11.149 | 8.391 | 1.329 | - |
| 512 | 32.483 | 16.745 | 1.940 | - |
| 1024 | 64.577 | 33.252 | 1.942 | - |

