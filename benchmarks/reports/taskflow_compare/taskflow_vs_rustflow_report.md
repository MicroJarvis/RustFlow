# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 1
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.069 | 0.065 | 1.055 | - |
| 911 | 2.817 | 1.332 | 2.115 | - |
| 3334 | 9.265 | 4.485 | 2.066 | - |
| 7311 | 18.426 | 9.064 | 2.033 | - |
| 12904 | 29.953 | 13.634 | 2.197 | - |

## Linear Chain

- Benchmark id: `linear_chain`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.023 | 0.003 | 7.570 | - |
| 4 | 0.013 | 0.010 | 1.304 | - |
| 8 | 0.030 | 0.010 | 3.037 | - |
| 16 | 0.020 | 0.010 | 2.042 | - |
| 32 | 0.028 | 0.012 | 2.361 | - |
| 64 | 0.036 | 0.007 | 5.125 | - |
| 128 | 0.040 | 0.012 | 3.309 | - |
| 256 | 0.064 | 0.016 | 3.982 | - |
| 512 | 0.098 | 0.034 | 2.885 | - |
| 1024 | 0.204 | 0.057 | 3.576 | - |
| 2048 | 0.341 | 0.096 | 3.552 | - |
| 4096 | 0.617 | 0.154 | 4.005 | - |
| 8192 | 1.164 | 0.293 | 3.971 | - |
| 16384 | 2.173 | 0.540 | 4.025 | - |
| 32768 | 4.599 | 1.211 | 3.798 | - |
| 65536 | 9.357 | 2.791 | 3.352 | - |

## Thread Pool

- Benchmark id: `thread_pool`
- Note: Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 22.814 | 6.228 | 3.663 | - |
| 50 | 11.078 | 2.910 | 3.807 | - |
| 20 | 4.502 | 1.083 | 4.157 | - |
| 10 | 2.237 | 0.506 | 4.421 | - |
| 5 | 1.152 | 0.274 | 4.203 | - |

## Wavefront

- Benchmark id: `wavefront`
- Note: Report profile caps the grid size at 256x256 blocks (original default: 16384x16384).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.016 | 0.005 | 3.225 | - |
| 4 | 0.014 | 0.005 | 2.817 | - |
| 16 | 0.035 | 0.007 | 4.952 | - |
| 64 | 0.046 | 0.011 | 4.189 | - |
| 256 | 0.116 | 0.035 | 3.310 | - |
| 1024 | 0.383 | 0.139 | 2.757 | - |

