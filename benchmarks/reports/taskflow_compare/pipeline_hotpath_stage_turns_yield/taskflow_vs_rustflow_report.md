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
| 2 | 0.054 | 0.030 | 1.766 | - |
| 4 | 0.075 | 0.030 | 2.475 | - |
| 8 | 0.139 | 0.047 | 2.942 | - |
| 16 | 0.272 | 0.057 | 4.740 | - |
| 32 | 0.488 | 0.092 | 5.304 | - |
| 64 | 0.975 | 0.177 | 5.517 | - |
| 128 | 1.823 | 0.319 | 5.709 | - |
| 256 | 3.391 | 0.546 | 6.207 | - |
| 512 | 6.214 | 1.010 | 6.154 | - |
| 1024 | 11.130 | 2.114 | 5.265 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.028 | 0.024 | 1.165 | - |
| 911 | 4.095 | 0.886 | 4.622 | - |
| 3334 | 14.978 | 2.954 | 5.071 | - |
| 7311 | 33.024 | 6.692 | 4.935 | - |
| 12904 | 57.423 | 11.241 | 5.109 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.260 | 0.153 | 1.697 | - |
| 4 | 0.492 | 0.192 | 2.566 | - |
| 8 | 0.964 | 0.329 | 2.931 | - |
| 16 | 1.922 | 0.563 | 3.414 | - |
| 32 | 3.840 | 1.049 | 3.660 | - |
| 64 | 7.661 | 2.061 | 3.717 | - |
| 128 | 15.260 | 4.165 | 3.664 | - |
| 256 | 30.495 | 8.337 | 3.658 | - |
| 512 | 61.016 | 16.627 | 3.670 | - |
| 1024 | 122.126 | 32.788 | 3.725 | - |

