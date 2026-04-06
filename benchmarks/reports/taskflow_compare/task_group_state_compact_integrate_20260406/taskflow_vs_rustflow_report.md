# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Integrate

- Benchmark id: `integrate`
- Note: Report profile caps the x-interval at 500 (original default: 2000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0.016 | 0.006 | 2.539 | - |
| 100 | 1.641 | 6.734 | 0.244 | - |
| 200 | 3.732 | 13.739 | 0.272 | - |
| 300 | 6.796 | 23.037 | 0.295 | - |
| 400 | 8.175 | 28.822 | 0.284 | - |
| 500 | 13.120 | 46.382 | 0.283 | - |

