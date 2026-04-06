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
| 2 | 0.059 | 0.067 | 0.876 | - |
| 4 | 0.067 | 0.082 | 0.823 | - |
| 8 | 0.115 | 0.065 | 1.785 | - |
| 16 | 0.202 | 0.069 | 2.930 | - |
| 32 | 0.352 | 0.124 | 2.840 | - |
| 64 | 0.689 | 0.222 | 3.109 | - |
| 128 | 1.352 | 0.390 | 3.470 | - |
| 256 | 2.201 | 0.794 | 2.771 | - |
| 512 | 3.869 | 1.538 | 2.516 | - |
| 1024 | 6.729 | 2.612 | 2.576 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.031 | 0.035 | 0.877 | - |
| 911 | 2.183 | 0.803 | 2.718 | - |
| 3334 | 7.772 | 2.622 | 2.964 | - |
| 7311 | 16.683 | 5.984 | 2.788 | - |
| 12904 | 29.719 | 10.486 | 2.834 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.155 | 0.171 | 0.908 | - |
| 4 | 0.280 | 0.198 | 1.419 | - |
| 8 | 0.495 | 0.319 | 1.552 | - |
| 16 | 1.041 | 0.566 | 1.841 | - |
| 32 | 2.066 | 1.093 | 1.890 | - |
| 64 | 4.051 | 2.158 | 1.878 | - |
| 128 | 8.120 | 4.196 | 1.935 | - |
| 256 | 16.100 | 8.398 | 1.917 | - |
| 512 | 32.375 | 16.644 | 1.945 | - |
| 1024 | 64.317 | 33.138 | 1.941 | - |

