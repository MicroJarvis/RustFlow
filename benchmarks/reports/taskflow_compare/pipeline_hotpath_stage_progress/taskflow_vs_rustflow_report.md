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
| 2 | 0.077 | 14.763 | 0.005 | - |
| 4 | 0.104 | 10.533 | 0.010 | - |
| 8 | 0.179 | 0.031 | 5.826 | - |
| 16 | 0.351 | 0.058 | 6.060 | - |
| 32 | 0.652 | 0.077 | 8.427 | - |
| 64 | 1.282 | 0.353 | 3.636 | - |
| 128 | 2.424 | 0.447 | 5.419 | - |
| 256 | 4.796 | 0.451 | 10.643 | - |
| 512 | 9.538 | 0.827 | 11.529 | - |
| 1024 | 17.520 | 7.584 | 2.310 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.072 | 0.115 | 0.628 | - |
| 911 | 12.785 | 5.870 | 2.178 | - |
| 3334 | 21.642 | 12.871 | 1.681 | - |
| 7311 | 51.763 | 27.830 | 1.860 | - |
| 12904 | 81.482 | 37.538 | 2.171 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.359 | 0.203 | 1.767 | - |
| 4 | 0.582 | 0.236 | 2.471 | - |
| 8 | 1.158 | 0.377 | 3.075 | - |
| 16 | 2.369 | 0.656 | 3.612 | - |
| 32 | 4.397 | 1.407 | 3.125 | - |
| 64 | 8.833 | 2.446 | 3.612 | - |
| 128 | 17.391 | 4.752 | 3.660 | - |
| 256 | 48.936 | 9.957 | 4.915 | - |
| 512 | 86.744 | 19.143 | 4.531 | - |
| 1024 | 151.261 | 39.498 | 3.830 | - |

