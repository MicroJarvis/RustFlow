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
| 2 | 0.078 | 0.052 | 1.509 | - |
| 4 | 0.104 | 0.043 | 2.420 | - |
| 8 | 0.190 | 0.044 | 4.359 | - |
| 16 | 0.345 | 0.071 | 4.887 | - |
| 32 | 0.638 | 0.161 | 3.952 | - |
| 64 | 1.248 | 0.289 | 4.314 | - |
| 128 | 2.194 | 0.443 | 4.953 | - |
| 256 | 3.842 | 0.734 | 5.231 | - |
| 512 | 6.563 | 1.547 | 4.242 | - |
| 1024 | 11.224 | 2.701 | 4.155 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.265 | 0.198 | 1.338 | - |
| 4 | 0.497 | 0.198 | 2.515 | - |
| 8 | 0.963 | 0.317 | 3.036 | - |
| 16 | 1.921 | 0.557 | 3.447 | - |
| 32 | 3.827 | 1.044 | 3.667 | - |
| 64 | 7.666 | 2.086 | 3.676 | - |
| 128 | 15.235 | 4.196 | 3.631 | - |
| 256 | 30.524 | 8.310 | 3.673 | - |
| 512 | 60.924 | 16.559 | 3.679 | - |
| 1024 | 121.686 | 32.969 | 3.691 | - |

