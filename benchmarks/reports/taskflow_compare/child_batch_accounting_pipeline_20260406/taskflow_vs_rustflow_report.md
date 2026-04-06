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
| 2 | 0.050 | 0.171 | 0.295 | - |
| 4 | 0.071 | 0.128 | 0.552 | - |
| 8 | 0.164 | 0.088 | 1.855 | - |
| 16 | 0.317 | 0.141 | 2.250 | - |
| 32 | 0.581 | 0.342 | 1.698 | - |
| 64 | 1.089 | 0.482 | 2.257 | - |
| 128 | 1.808 | 0.651 | 2.779 | - |
| 256 | 3.707 | 0.981 | 3.778 | - |
| 512 | 6.544 | 1.688 | 3.877 | - |
| 1024 | 11.419 | 2.615 | 4.367 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.031 | 0.035 | 0.897 | - |
| 911 | 3.951 | 0.781 | 5.056 | - |
| 3334 | 14.423 | 2.609 | 5.529 | - |
| 7311 | 31.254 | 5.903 | 5.295 | - |
| 12904 | 55.062 | 10.145 | 5.427 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.232 | 0.206 | 1.129 | - |
| 4 | 0.541 | 0.192 | 2.812 | - |
| 8 | 0.996 | 0.313 | 3.181 | - |
| 16 | 1.984 | 0.564 | 3.519 | - |
| 32 | 3.972 | 1.066 | 3.727 | - |
| 64 | 8.003 | 2.133 | 3.753 | - |
| 128 | 15.813 | 4.190 | 3.774 | - |
| 256 | 31.357 | 8.335 | 3.762 | - |
| 512 | 63.163 | 16.552 | 3.816 | - |
| 1024 | 126.234 | 33.258 | 3.796 | - |

