# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 5
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.030 | 0.050 | 0.591 | - |
| 911 | 3.849 | 1.765 | 2.180 | - |
| 3334 | 14.659 | 4.341 | 3.377 | - |
| 7311 | 31.696 | 6.683 | 4.743 | - |
| 12904 | 54.993 | 9.807 | 5.608 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.328 | 0.182 | 1.804 | - |
| 4 | 0.620 | 0.231 | 2.686 | - |
| 8 | 1.227 | 0.383 | 3.203 | - |
| 16 | 2.394 | 0.690 | 3.469 | - |
| 32 | 4.721 | 1.288 | 3.665 | - |
| 64 | 9.543 | 2.483 | 3.843 | - |
| 128 | 18.981 | 4.881 | 3.889 | - |
| 256 | 37.987 | 9.677 | 3.926 | - |
| 512 | 76.258 | 19.286 | 3.954 | - |
| 1024 | 152.653 | 38.443 | 3.971 | - |

