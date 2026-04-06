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
| 2 | 0.275 | 0.212 | 1.294 | - |
| 4 | 0.573 | 0.261 | 2.198 | - |
| 8 | 1.125 | 0.436 | 2.579 | - |
| 16 | 2.233 | 0.793 | 2.816 | - |
| 32 | 4.422 | 1.459 | 3.030 | - |
| 64 | 8.809 | 2.691 | 3.274 | - |
| 128 | 17.503 | 5.046 | 3.469 | - |
| 256 | 35.929 | 9.706 | 3.702 | - |
| 512 | 73.999 | 18.746 | 3.947 | - |
| 1024 | 144.675 | 36.400 | 3.975 | - |

