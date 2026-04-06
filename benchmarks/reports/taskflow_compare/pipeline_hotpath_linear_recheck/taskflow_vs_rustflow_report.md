# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 1.949 | 11.487 | 0.170 | - |
| 4 | 0.666 | 3.699 | 0.180 | - |
| 8 | 1.519 | 6.708 | 0.226 | - |
| 16 | 2.425 | 5.632 | 0.431 | - |
| 32 | 6.435 | 5.742 | 1.121 | - |
| 64 | 34.606 | 14.674 | 2.358 | - |
| 128 | 19.556 | 37.647 | 0.519 | - |
| 256 | 195.043 | 21.731 | 8.975 | - |
| 512 | 539.247 | 36.400 | 14.815 | - |
| 1024 | 850.508 | 69.196 | 12.291 | - |

