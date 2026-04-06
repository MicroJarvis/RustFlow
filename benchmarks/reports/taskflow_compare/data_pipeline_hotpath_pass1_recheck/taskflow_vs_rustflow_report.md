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
| 2 | 0.073 | 0.041 | 1.778 | - |
| 4 | 0.100 | 0.032 | 3.173 | - |
| 8 | 0.193 | 0.043 | 4.454 | - |
| 16 | 0.356 | 0.063 | 5.626 | - |
| 32 | 0.643 | 0.111 | 5.779 | - |
| 64 | 1.245 | 0.213 | 5.845 | - |
| 128 | 2.182 | 0.355 | 6.151 | - |
| 256 | 3.841 | 0.825 | 4.658 | - |
| 512 | 6.557 | 1.647 | 3.981 | - |
| 1024 | 11.283 | 2.533 | 4.454 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.266 | 0.155 | 1.710 | - |
| 4 | 0.494 | 0.199 | 2.488 | - |
| 8 | 0.979 | 0.329 | 2.978 | - |
| 16 | 1.922 | 0.595 | 3.231 | - |
| 32 | 3.847 | 1.113 | 3.455 | - |
| 64 | 7.615 | 2.130 | 3.575 | - |
| 128 | 15.226 | 4.201 | 3.624 | - |
| 256 | 30.490 | 8.313 | 3.668 | - |
| 512 | 60.936 | 16.536 | 3.685 | - |
| 1024 | 121.964 | 32.979 | 3.698 | - |

