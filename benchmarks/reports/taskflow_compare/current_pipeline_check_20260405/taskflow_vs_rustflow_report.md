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
| 2 | 0.056 | 0.061 | 0.918 | - |
| 4 | 0.099 | 0.041 | 2.426 | - |
| 8 | 0.175 | 0.051 | 3.458 | - |
| 16 | 0.340 | 0.075 | 4.518 | - |
| 32 | 0.649 | 0.152 | 4.269 | - |
| 64 | 1.272 | 0.288 | 4.416 | - |
| 128 | 2.243 | 0.450 | 4.985 | - |
| 256 | 3.938 | 0.870 | 4.528 | - |
| 512 | 6.564 | 1.511 | 4.344 | - |
| 1024 | 10.745 | 2.686 | 4.001 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.030 | 0.038 | 0.782 | - |
| 911 | 3.830 | 1.804 | 2.123 | - |
| 3334 | 14.452 | 4.469 | 3.234 | - |
| 7311 | 31.075 | 8.868 | 3.504 | - |
| 12904 | 55.297 | 10.406 | 5.314 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.222 | 0.216 | 1.028 | - |
| 4 | 0.504 | 0.229 | 2.202 | - |
| 8 | 0.995 | 0.387 | 2.572 | - |
| 16 | 1.928 | 0.631 | 3.057 | - |
| 32 | 3.831 | 1.294 | 2.962 | - |
| 64 | 7.647 | 2.410 | 3.172 | - |
| 128 | 15.265 | 4.509 | 3.385 | - |
| 256 | 30.512 | 8.616 | 3.541 | - |
| 512 | 61.166 | 16.335 | 3.744 | - |
| 1024 | 125.112 | 32.851 | 3.808 | - |

