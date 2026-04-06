# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.029 | 0.050 | 0.583 | - |
| 911 | 3.836 | 1.927 | 1.991 | - |
| 3334 | 14.614 | 5.007 | 2.919 | - |
| 7311 | 32.220 | 7.351 | 4.383 | - |
| 12904 | 54.834 | 11.376 | 4.820 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.297 | 0.187 | 1.593 | - |
| 4 | 0.578 | 0.233 | 2.481 | - |
| 8 | 1.122 | 0.384 | 2.918 | - |
| 16 | 2.221 | 0.688 | 3.229 | - |
| 32 | 4.419 | 1.288 | 3.430 | - |
| 64 | 9.013 | 2.498 | 3.608 | - |
| 128 | 19.047 | 4.916 | 3.875 | - |
| 256 | 36.285 | 9.727 | 3.730 | - |
| 512 | 70.344 | 19.133 | 3.677 | - |
| 1024 | 143.251 | 36.507 | 3.924 | - |

