# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.021 | 0.018 | 1.188 | - |
| 2 | 0.011 | 0.025 | 0.432 | - |
| 3 | 0.016 | 0.225 | 0.072 | - |
| 4 | 0.046 | 0.342 | 0.134 | - |

