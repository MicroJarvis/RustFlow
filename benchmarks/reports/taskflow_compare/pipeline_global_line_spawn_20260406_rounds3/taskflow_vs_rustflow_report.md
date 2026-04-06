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
| 2 | 0.050 | 0.030 | 1.675 | - |
| 4 | 0.058 | 0.018 | 3.235 | - |
| 8 | 0.095 | 0.021 | 4.598 | - |
| 16 | 0.178 | 0.077 | 2.307 | - |
| 32 | 0.335 | 0.144 | 2.332 | - |
| 64 | 0.607 | 0.256 | 2.367 | - |
| 128 | 1.163 | 0.401 | 2.900 | - |
| 256 | 2.162 | 0.937 | 2.308 | - |
| 512 | 3.792 | 1.804 | 2.102 | - |
| 1024 | 6.336 | 2.450 | 2.586 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.029 | 0.026 | 1.112 | - |
| 911 | 2.269 | 0.855 | 2.652 | - |
| 3334 | 7.774 | 2.895 | 2.686 | - |
| 7311 | 16.697 | 5.815 | 2.871 | - |
| 12904 | 29.308 | 10.171 | 2.881 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.155 | 0.167 | 0.929 | - |
| 4 | 0.286 | 0.202 | 1.416 | - |
| 8 | 0.542 | 0.326 | 1.663 | - |
| 16 | 1.049 | 0.568 | 1.847 | - |
| 32 | 2.105 | 1.046 | 2.013 | - |
| 64 | 4.141 | 2.082 | 1.989 | - |
| 128 | 8.160 | 4.238 | 1.925 | - |
| 256 | 16.287 | 8.300 | 1.962 | - |
| 512 | 32.409 | 16.631 | 1.949 | - |
| 1024 | 64.502 | 33.283 | 1.938 | - |

