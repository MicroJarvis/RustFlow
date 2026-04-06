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
| 2 | 0.052 | 0.041 | 1.257 | - |
| 4 | 0.059 | 0.032 | 1.874 | - |
| 8 | 0.103 | 0.036 | 2.885 | - |
| 16 | 0.179 | 0.055 | 3.255 | - |
| 32 | 0.332 | 0.097 | 3.413 | - |
| 64 | 0.652 | 0.219 | 2.981 | - |
| 128 | 1.149 | 0.451 | 2.548 | - |
| 256 | 2.149 | 0.882 | 2.438 | - |
| 512 | 3.806 | 1.411 | 2.698 | - |
| 1024 | 6.649 | 2.892 | 2.299 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.032 | 0.036 | 0.882 | - |
| 911 | 2.152 | 0.865 | 2.486 | - |
| 3334 | 7.764 | 2.683 | 2.893 | - |
| 7311 | 16.806 | 5.919 | 2.839 | - |
| 12904 | 28.739 | 10.183 | 2.822 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.153 | 0.182 | 0.838 | - |
| 4 | 0.286 | 0.185 | 1.544 | - |
| 8 | 0.530 | 0.316 | 1.677 | - |
| 16 | 1.041 | 0.561 | 1.855 | - |
| 32 | 2.053 | 1.059 | 1.938 | - |
| 64 | 4.065 | 2.064 | 1.969 | - |
| 128 | 8.111 | 4.219 | 1.923 | - |
| 256 | 16.236 | 8.349 | 1.945 | - |
| 512 | 32.290 | 16.638 | 1.941 | - |
| 1024 | 64.182 | 33.354 | 1.924 | - |

