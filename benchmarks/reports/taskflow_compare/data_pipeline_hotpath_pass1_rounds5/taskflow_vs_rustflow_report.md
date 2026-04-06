# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.058 | 0.035 | 1.663 | - |
| 4 | 0.082 | 0.030 | 2.738 | - |
| 8 | 0.154 | 0.043 | 3.601 | - |
| 16 | 0.281 | 0.064 | 4.411 | - |
| 32 | 0.530 | 0.106 | 5.005 | - |
| 64 | 1.012 | 0.184 | 5.495 | - |
| 128 | 1.828 | 0.339 | 5.386 | - |
| 256 | 3.297 | 0.768 | 4.292 | - |
| 512 | 5.734 | 1.432 | 4.004 | - |
| 1024 | 10.474 | 2.372 | 4.415 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.260 | 0.152 | 1.709 | - |
| 4 | 0.502 | 0.203 | 2.479 | - |
| 8 | 0.978 | 0.330 | 2.967 | - |
| 16 | 1.930 | 0.593 | 3.253 | - |
| 32 | 3.828 | 1.096 | 3.491 | - |
| 64 | 7.623 | 2.131 | 3.577 | - |
| 128 | 15.226 | 4.191 | 3.633 | - |
| 256 | 30.447 | 8.311 | 3.663 | - |
| 512 | 60.950 | 16.546 | 3.684 | - |
| 1024 | 121.911 | 32.935 | 3.702 | - |

