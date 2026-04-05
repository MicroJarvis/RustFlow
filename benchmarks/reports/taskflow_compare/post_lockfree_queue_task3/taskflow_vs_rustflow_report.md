# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Binary Tree

- Benchmark id: `binary_tree`
- Note: Report profile caps the original Taskflow sweep at 14 layers (original default: 25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.031 | 0.004 | 7.061 | - |
| 4 | 0.021 | 0.004 | 5.215 | - |
| 8 | 0.023 | 0.004 | 6.144 | - |
| 16 | 0.031 | 0.005 | 6.114 | - |
| 32 | 0.025 | 0.017 | 1.445 | - |
| 64 | 0.038 | 0.010 | 3.756 | - |
| 128 | 0.070 | 0.034 | 2.055 | - |
| 256 | 0.138 | 0.039 | 3.502 | - |
| 512 | 0.261 | 0.052 | 5.052 | - |
| 1024 | 0.475 | 0.121 | 3.911 | - |
| 2048 | 0.836 | 0.263 | 3.175 | - |
| 4096 | 1.586 | 0.503 | 3.154 | - |
| 8192 | 3.440 | 1.018 | 3.380 | - |
| 16384 | 6.367 | 1.805 | 3.527 | - |

## Graph Traversal

- Benchmark id: `graph_traversal`
- Note: Report profile caps the graph dimension at 64 (original default: 2048).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.014 | 0.012 | 1.180 | - |
| 12 | 0.009 | 0.014 | 0.637 | - |
| 53 | 0.013 | 0.013 | 1.025 | - |
| 219 | 0.032 | 0.035 | 0.936 | - |
| 911 | 0.079 | 0.077 | 1.032 | - |
| 3548 | 0.238 | 0.255 | 0.932 | - |
| 14187 | 0.851 | 0.960 | 0.887 | - |

## Linear Chain

- Benchmark id: `linear_chain`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.019 | 0.015 | 1.330 | - |
| 4 | 0.016 | 0.013 | 1.224 | - |
| 8 | 0.020 | 0.009 | 2.173 | - |
| 16 | 0.026 | 0.012 | 2.126 | - |
| 32 | 0.020 | 0.019 | 1.021 | - |
| 64 | 0.025 | 0.017 | 1.518 | - |
| 128 | 0.025 | 0.036 | 0.700 | - |
| 256 | 0.045 | 0.026 | 1.704 | - |
| 512 | 0.081 | 0.027 | 3.029 | - |
| 1024 | 0.142 | 0.053 | 2.695 | - |
| 2048 | 0.268 | 0.081 | 3.320 | - |
| 4096 | 0.510 | 0.143 | 3.566 | - |
| 8192 | 0.991 | 0.279 | 3.550 | - |
| 16384 | 1.931 | 0.559 | 3.456 | - |
| 32768 | 4.471 | 1.123 | 3.982 | - |
| 65536 | 9.428 | 2.712 | 3.477 | - |

## Thread Pool

- Benchmark id: `thread_pool`
- Note: Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 36.994 | 5.557 | 6.657 | - |
| 50 | 18.405 | 2.387 | 7.709 | - |
| 20 | 7.342 | 0.981 | 7.487 | - |
| 10 | 3.739 | 0.521 | 7.181 | - |
| 5 | 1.832 | 0.265 | 6.905 | - |

## Wavefront

- Benchmark id: `wavefront`
- Note: Report profile caps the grid size at 256x256 blocks (original default: 16384x16384).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.007 | 0.013 | 0.521 | - |
| 4 | 0.010 | 0.007 | 1.440 | - |
| 16 | 0.020 | 0.008 | 2.467 | - |
| 64 | 0.042 | 0.012 | 3.484 | - |
| 256 | 0.139 | 0.032 | 4.375 | - |
| 1024 | 0.516 | 0.120 | 4.301 | - |

