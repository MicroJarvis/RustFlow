# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Async Task

- Benchmark id: `async_task`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^21).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.006 | 0.000 | 18.959 | - |
| 2 | 0.006 | 0.002 | 2.417 | - |
| 4 | 0.003 | 0.011 | 0.230 | - |
| 8 | 0.005 | 0.001 | 5.180 | - |
| 16 | 0.009 | 0.007 | 1.333 | - |
| 32 | 0.014 | 0.005 | 2.622 | - |
| 64 | 0.030 | 0.016 | 1.832 | - |
| 128 | 0.066 | 0.025 | 2.673 | - |
| 256 | 0.126 | 0.041 | 3.060 | - |
| 512 | 0.183 | 0.066 | 2.766 | - |
| 1024 | 0.347 | 0.128 | 2.710 | - |
| 2048 | 0.723 | 0.449 | 1.612 | - |
| 4096 | 1.181 | 0.893 | 1.322 | - |
| 8192 | 2.311 | 1.935 | 1.195 | - |
| 16384 | 4.635 | 4.270 | 1.086 | - |
| 32768 | 9.326 | 8.423 | 1.107 | - |
| 65536 | 15.440 | 16.228 | 0.951 | - |

## Binary Tree

- Benchmark id: `binary_tree`
- Note: Report profile caps the original Taskflow sweep at 14 layers (original default: 25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.015 | 0.010 | 1.575 | - |
| 4 | 0.018 | 0.010 | 1.847 | - |
| 8 | 0.013 | 0.010 | 1.322 | - |
| 16 | 0.016 | 0.010 | 1.500 | - |
| 32 | 0.023 | 0.010 | 2.254 | - |
| 64 | 0.043 | 0.016 | 2.702 | - |
| 128 | 0.065 | 0.019 | 3.488 | - |
| 256 | 0.132 | 0.035 | 3.805 | - |
| 512 | 0.260 | 0.071 | 3.650 | - |
| 1024 | 0.456 | 0.143 | 3.179 | - |
| 2048 | 0.907 | 0.279 | 3.251 | - |
| 4096 | 1.704 | 0.552 | 3.088 | - |
| 8192 | 3.395 | 0.945 | 3.593 | - |
| 16384 | 5.576 | 2.180 | 2.557 | - |

## Embarrassing Parallelism

- Benchmark id: `embarrassing_parallelism`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^20).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.016 | 0.011 | 1.483 | - |
| 2 | 0.014 | 0.008 | 1.694 | - |
| 4 | 0.010 | 0.020 | 0.520 | - |
| 8 | 0.015 | 0.010 | 1.483 | - |
| 16 | 0.021 | 0.015 | 1.460 | - |
| 32 | 0.026 | 0.017 | 1.549 | - |
| 64 | 0.045 | 0.026 | 1.747 | - |
| 128 | 0.078 | 0.049 | 1.584 | - |
| 256 | 0.136 | 0.066 | 2.045 | - |
| 512 | 0.242 | 0.131 | 1.851 | - |
| 1024 | 0.460 | 0.238 | 1.934 | - |
| 2048 | 0.835 | 0.484 | 1.726 | - |
| 4096 | 1.640 | 0.927 | 1.769 | - |
| 8192 | 3.240 | 1.870 | 1.733 | - |
| 16384 | 6.366 | 3.429 | 1.856 | - |
| 32768 | 12.812 | 7.079 | 1.810 | - |
| 65536 | 24.868 | 14.752 | 1.686 | - |

## Linear Chain

- Benchmark id: `linear_chain`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.007 | 0.008 | 0.957 | - |
| 4 | 0.007 | 0.009 | 0.796 | - |
| 8 | 0.009 | 0.009 | 1.014 | - |
| 16 | 0.021 | 0.007 | 2.814 | - |
| 32 | 0.024 | 0.015 | 1.662 | - |
| 64 | 0.018 | 0.016 | 1.149 | - |
| 128 | 0.027 | 0.021 | 1.300 | - |
| 256 | 0.039 | 0.017 | 2.292 | - |
| 512 | 0.081 | 0.040 | 1.999 | - |
| 1024 | 0.136 | 0.052 | 2.632 | - |
| 2048 | 0.257 | 0.086 | 2.973 | - |
| 4096 | 0.488 | 0.154 | 3.164 | - |
| 8192 | 0.948 | 0.273 | 3.478 | - |
| 16384 | 1.926 | 0.537 | 3.587 | - |
| 32768 | 4.163 | 1.088 | 3.828 | - |
| 65536 | 8.755 | 2.363 | 3.705 | - |

## Thread Pool

- Benchmark id: `thread_pool`
- Note: Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 36.734 | 5.603 | 6.556 | - |
| 50 | 18.326 | 2.681 | 6.836 | - |
| 20 | 7.384 | 1.031 | 7.162 | - |
| 10 | 3.701 | 0.570 | 6.492 | - |
| 5 | 1.844 | 0.267 | 6.907 | - |

