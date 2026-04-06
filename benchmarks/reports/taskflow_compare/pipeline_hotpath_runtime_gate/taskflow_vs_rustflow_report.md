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
| 2 | 0.069 | 3.417 | 0.020 | - |
| 4 | 0.106 | 2.997 | 0.035 | - |
| 8 | 0.182 | 3.102 | 0.059 | - |
| 16 | 0.350 | 0.227 | 1.537 | - |
| 32 | 0.654 | 0.082 | 7.944 | - |
| 64 | 1.257 | 2.234 | 0.563 | - |
| 128 | 2.412 | 2.545 | 0.948 | - |
| 256 | 4.783 | 5.776 | 0.828 | - |
| 512 | 9.517 | 5.835 | 1.631 | - |
| 1024 | 17.353 | 8.732 | 1.987 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.075 | 1.854 | 0.040 | - |
| 911 | 10.725 | 3.215 | 3.336 | - |
| 3334 | 38.721 | 6.470 | 5.985 | - |
| 7311 | 73.794 | 30.883 | 2.389 | - |
| 12904 | 155.689 | 44.419 | 3.505 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.338 | 8.183 | 0.041 | - |
| 4 | 0.654 | 17.594 | 0.037 | - |
| 8 | 1.159 | 12.796 | 0.091 | - |
| 16 | 2.271 | 5.196 | 0.437 | - |
| 32 | 4.618 | 3.410 | 1.355 | - |
| 64 | 9.370 | 9.004 | 1.041 | - |
| 128 | 58.014 | 9.275 | 6.255 | - |
| 256 | 115.887 | 18.848 | 6.148 | - |
| 512 | 245.579 | 41.542 | 5.912 | - |
| 1024 | 665.553 | 58.304 | 11.415 | - |

