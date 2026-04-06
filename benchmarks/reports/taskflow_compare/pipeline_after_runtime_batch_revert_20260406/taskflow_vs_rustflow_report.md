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
| 2 | 0.034 | 0.046 | 0.741 | - |
| 4 | 0.054 | 0.017 | 3.190 | - |
| 8 | 0.118 | 0.026 | 4.595 | - |
| 16 | 0.228 | 0.046 | 4.952 | - |
| 32 | 0.496 | 0.065 | 7.627 | - |
| 64 | 1.145 | 0.185 | 6.203 | - |
| 128 | 2.033 | 0.648 | 3.136 | - |
| 256 | 3.812 | 1.218 | 3.130 | - |
| 512 | 6.462 | 1.761 | 3.670 | - |
| 1024 | 11.963 | 2.817 | 4.246 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.030 | 0.038 | 0.781 | - |
| 911 | 4.142 | 1.026 | 4.039 | - |
| 3334 | 15.614 | 3.678 | 4.245 | - |
| 7311 | 33.528 | 6.977 | 4.806 | - |
| 12904 | 58.734 | 11.163 | 5.261 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.241 | 0.230 | 1.049 | - |
| 4 | 0.545 | 0.202 | 2.697 | - |
| 8 | 1.029 | 0.342 | 3.009 | - |
| 16 | 2.062 | 0.590 | 3.493 | - |
| 32 | 4.021 | 1.064 | 3.779 | - |
| 64 | 8.127 | 2.073 | 3.921 | - |
| 128 | 16.253 | 4.208 | 3.863 | - |
| 256 | 32.225 | 8.356 | 3.856 | - |
| 512 | 64.324 | 16.549 | 3.887 | - |
| 1024 | 125.316 | 33.254 | 3.768 | - |

