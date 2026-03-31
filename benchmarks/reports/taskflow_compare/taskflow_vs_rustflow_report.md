# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 1
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Async Task

- Benchmark id: `async_task`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^21).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.057 | 0.003 | 18.875 | - |
| 2 | 0.032 | 0.004 | 8.031 | - |
| 4 | 0.070 | 0.005 | 13.975 | - |
| 8 | 0.080 | 0.008 | 10.000 | - |
| 16 | 0.061 | 0.010 | 6.079 | - |
| 32 | 0.111 | 0.039 | 2.857 | - |
| 64 | 0.292 | 0.027 | 10.809 | - |
| 128 | 0.290 | 0.112 | 2.593 | - |
| 256 | 0.786 | 0.193 | 4.071 | - |
| 512 | 1.592 | 0.352 | 4.521 | - |
| 1024 | 2.540 | 0.626 | 4.057 | - |
| 2048 | 5.729 | 1.229 | 4.661 | - |
| 4096 | 11.332 | 2.482 | 4.566 | - |
| 8192 | 19.531 | 4.184 | 4.668 | - |
| 16384 | 40.006 | 11.809 | 3.388 | - |
| 32768 | 79.958 | 14.515 | 5.509 | - |
| 65536 | 155.513 | 22.263 | 6.985 | - |

## Binary Tree

- Benchmark id: `binary_tree`
- Note: Report profile caps the original Taskflow sweep at 14 layers (original default: 25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.031 | 0.024 | 1.280 | - |
| 4 | 0.022 | 0.008 | 2.781 | - |
| 8 | 0.050 | 0.006 | 8.327 | - |
| 16 | 0.048 | 0.011 | 4.398 | - |
| 32 | 0.051 | 0.009 | 5.611 | - |
| 64 | 0.063 | 0.012 | 5.278 | - |
| 128 | 0.073 | 0.021 | 3.480 | - |
| 256 | 0.158 | 0.052 | 3.029 | - |
| 512 | 0.227 | 0.090 | 2.527 | - |
| 1024 | 0.391 | 0.189 | 2.070 | - |
| 2048 | 0.738 | 0.186 | 3.970 | - |
| 4096 | 1.553 | 0.716 | 2.169 | - |
| 8192 | 2.624 | 0.791 | 3.317 | - |
| 16384 | 5.141 | 1.521 | 3.380 | - |

## Black-Scholes

- Benchmark id: `black_scholes`
- Note: Report profile caps the original Taskflow sweep at 3,000 options (original default: 10,000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1000 | 96.544 | 6.859 | 14.076 | - |
| 2000 | 267.272 | 21.661 | 12.339 | - |
| 3000 | 508.721 | 43.985 | 11.566 | - |

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.076 | 0.077 | 0.982 | - |
| 4 | 0.086 | 0.025 | 3.458 | - |
| 8 | 0.128 | 0.039 | 3.282 | - |
| 16 | 0.146 | 0.051 | 2.865 | - |
| 32 | 0.265 | 0.093 | 2.846 | - |
| 64 | 0.448 | 0.166 | 2.699 | - |
| 128 | 0.848 | 0.318 | 2.667 | - |
| 256 | 1.633 | 0.631 | 2.588 | - |
| 512 | 3.142 | 1.094 | 2.872 | - |
| 1024 | 7.039 | 2.285 | 3.081 | - |

## Embarrassing Parallelism

- Benchmark id: `embarrassing_parallelism`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^20).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.027 | 0.012 | 2.246 | - |
| 2 | 0.021 | 0.009 | 2.375 | - |
| 4 | 0.020 | 0.013 | 1.503 | - |
| 8 | 0.035 | 0.014 | 2.500 | - |
| 16 | 0.040 | 0.016 | 2.526 | - |
| 32 | 0.048 | 0.023 | 2.094 | - |
| 64 | 0.078 | 0.032 | 2.438 | - |
| 128 | 0.078 | 0.048 | 1.633 | - |
| 256 | 0.201 | 0.089 | 2.254 | - |
| 512 | 0.334 | 0.166 | 2.015 | - |
| 1024 | 0.635 | 0.336 | 1.891 | - |
| 2048 | 1.235 | 0.652 | 1.893 | - |
| 4096 | 2.408 | 1.388 | 1.735 | - |
| 8192 | 4.455 | 2.733 | 1.630 | - |
| 16384 | 9.337 | 5.359 | 1.742 | - |
| 32768 | 16.994 | 10.891 | 1.560 | - |
| 65536 | 34.569 | 20.795 | 1.662 | - |

## Fibonacci

- Benchmark id: `fibonacci`
- Note: Report profile caps the original Taskflow sweep at `fib(20)` (original default: `fib(40)`).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.024 | 0.007 | 3.411 | - |
| 2 | 0.016 | 0.006 | 2.680 | - |
| 3 | 0.037 | 0.008 | 4.609 | - |
| 4 | 0.028 | 0.017 | 1.659 | - |
| 5 | 0.013 | 0.018 | 0.697 | - |
| 6 | 0.032 | 0.014 | 2.283 | - |
| 7 | 0.041 | 0.013 | 3.141 | - |
| 8 | 0.043 | 0.022 | 1.941 | - |
| 9 | 0.044 | 0.030 | 1.481 | - |
| 10 | 0.138 | 0.038 | 3.641 | - |
| 11 | 0.262 | 0.055 | 4.756 | - |
| 12 | 0.242 | 0.088 | 2.752 | - |
| 13 | 0.427 | 0.147 | 2.903 | - |
| 14 | 0.822 | 0.203 | 4.051 | - |
| 15 | 1.301 | 0.241 | 5.400 | - |
| 16 | 2.153 | 0.052 | 41.411 | - |
| 17 | 3.902 | 0.069 | 56.557 | - |
| 18 | 5.743 | 0.099 | 58.011 | - |
| 19 | 7.628 | 0.154 | 49.533 | - |
| 20 | 13.491 | 0.243 | 55.517 | - |

## For Each

- Benchmark id: `for_each`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.050 | 0.021 | 2.371 | - |
| 100 | 0.070 | 0.030 | 2.329 | - |
| 1000 | 0.160 | 0.032 | 5.012 | - |
| 10000 | 0.599 | 0.044 | 13.604 | - |
| 100000 | 4.874 | 0.198 | 24.615 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.096 | 0.051 | 1.877 | - |
| 911 | 2.424 | 1.319 | 1.837 | - |
| 3334 | 7.273 | 3.526 | 2.063 | - |
| 7311 | 15.796 | 7.737 | 2.042 | - |
| 12904 | 29.274 | 15.381 | 1.903 | - |

## Graph Traversal

- Benchmark id: `graph_traversal`
- Note: Report profile caps the graph dimension at 64 (original default: 2048).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.012 | 0.008 | 1.557 | - |
| 12 | 0.018 | 0.013 | 1.365 | - |
| 53 | 0.039 | 0.020 | 1.935 | - |
| 219 | 0.056 | 0.048 | 1.164 | - |
| 911 | 0.183 | 0.143 | 1.280 | - |
| 3548 | 0.536 | 0.429 | 1.249 | - |
| 14187 | 1.939 | 1.500 | 1.293 | - |

## Heterogeneous Traversal

- Benchmark id: `hetero_traversal`
- Note: Skipped: Taskflow benchmark is CUDA-based and RustFlow currently has no heterogeneous execution backend.

## Integrate

- Benchmark id: `integrate`
- Note: Report profile caps the x-interval at 500 (original default: 2000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0.014 | 0.005 | 2.750 | - |
| 100 | 547.243 | 8.863 | 61.745 | - |
| 200 | 1102.517 | 19.081 | 57.781 | - |
| 300 | 1976.799 | 34.606 | 57.123 | - |
| 400 | 2638.120 | 41.832 | 63.065 | - |
| 500 | 4914.862 | 68.123 | 72.147 | - |

## Linear Chain

- Benchmark id: `linear_chain`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.059 | 0.011 | 5.379 | - |
| 4 | 0.026 | 0.012 | 2.174 | - |
| 8 | 0.042 | 0.010 | 4.200 | - |
| 16 | 0.019 | 0.011 | 1.769 | - |
| 32 | 0.046 | 0.013 | 3.574 | - |
| 64 | 0.081 | 0.032 | 2.527 | - |
| 128 | 0.137 | 0.016 | 8.550 | - |
| 256 | 0.212 | 0.024 | 8.854 | - |
| 512 | 0.661 | 0.043 | 15.374 | - |
| 1024 | 0.833 | 0.076 | 10.960 | - |
| 2048 | 1.391 | 0.143 | 9.728 | - |
| 4096 | 2.945 | 0.259 | 11.372 | - |
| 8192 | 5.081 | 0.530 | 9.588 | - |
| 16384 | 10.045 | 0.974 | 10.313 | - |
| 32768 | 19.851 | 2.082 | 9.534 | - |
| 65536 | 33.556 | 4.168 | 8.051 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.869 | 0.305 | 2.848 | - |
| 4 | 0.201 | 0.202 | 0.995 | - |
| 8 | 0.325 | 0.302 | 1.075 | - |
| 16 | 0.555 | 0.566 | 0.981 | - |
| 32 | 1.074 | 1.057 | 1.016 | - |
| 64 | 2.086 | 2.035 | 1.025 | - |
| 128 | 4.080 | 3.949 | 1.033 | - |
| 256 | 7.958 | 8.215 | 0.969 | - |
| 512 | 16.178 | 16.218 | 0.998 | - |
| 1024 | 31.913 | 31.816 | 1.003 | - |

## Mandelbrot

- Benchmark id: `mandelbrot`
- Note: Report profile caps the image size at 300x300 (original default: 1000x1000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 5.948 | 5.960 | 0.998 | - |
| 200 | 23.330 | 23.521 | 0.992 | - |
| 300 | 52.447 | 52.709 | 0.995 | - |

## Matrix Multiplication

- Benchmark id: `matrix_multiplication`
- Note: Report profile caps matrix size at 256 (original default: 1024).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 128 | 0.442 | 0.785 | 0.563 | - |
| 192 | 1.493 | 2.024 | 0.738 | - |
| 256 | 3.643 | 6.369 | 0.572 | - |

## Merge Sort

- Benchmark id: `merge_sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.024 | 0.013 | 1.878 | - |
| 100 | 0.017 | 0.014 | 1.244 | - |
| 1000 | 0.046 | 0.030 | 1.519 | - |
| 10000 | 0.451 | 0.199 | 2.268 | - |
| 100000 | 4.459 | 2.154 | 2.070 | - |

## MNIST

- Benchmark id: `mnist`
- Note: Skipped: Taskflow benchmark requires external Kaggle MNIST data files that are not present in this workspace.

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.017 | 0.009 | 1.884 | - |
| 2 | 0.034 | 0.010 | 3.358 | - |
| 3 | 0.019 | 0.015 | 1.264 | - |
| 4 | 0.038 | 0.052 | 0.739 | - |
| 5 | 0.101 | 0.080 | 1.257 | - |
| 6 | 0.171 | 0.222 | 0.769 | - |
| 7 | 0.552 | 0.471 | 1.172 | - |
| 8 | 1.501 | 1.038 | 1.446 | - |
| 9 | 6.446 | 1.395 | 4.621 | - |
| 10 | 33.107 | 5.504 | 6.015 | - |

## Primes

- Benchmark id: `primes`
- Note: Report profile caps the original Taskflow sweep at 10^5 (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.033 | 0.023 | 1.446 | - |
| 100 | 0.037 | 0.011 | 3.326 | - |
| 1000 | 0.078 | 0.021 | 3.716 | - |
| 10000 | 0.638 | 0.085 | 7.501 | - |
| 100000 | 5.503 | 0.676 | 8.141 | - |

## Reduce Sum

- Benchmark id: `reduce_sum`
- Note: Report profile caps the original Taskflow sweep at 10^6 elements (original default: 10^9).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.041 | 0.015 | 2.764 | - |
| 100 | 0.051 | 0.011 | 4.674 | - |
| 1000 | 0.048 | 0.019 | 2.539 | - |
| 10000 | 0.045 | 0.022 | 2.045 | - |
| 100000 | 0.058 | 0.047 | 1.235 | - |
| 1000000 | 0.295 | 0.297 | 0.992 | - |

## Scan

- Benchmark id: `scan`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.076 | 0.014 | 5.426 | - |
| 100 | 0.194 | 0.019 | 10.228 | - |
| 1000 | 0.248 | 0.009 | 27.551 | - |
| 10000 | 0.326 | 0.022 | 14.835 | - |
| 100000 | 1.113 | 0.079 | 14.088 | - |

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.031 | 0.010 | 3.146 | - |
| 2 | 0.110 | 0.043 | 2.552 | - |
| 3 | 0.911 | 0.394 | 2.312 | - |
| 4 | 5.639 | 0.415 | 13.589 | - |

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.586 | 0.004 | 146.573 | - |
| 100 | 0.238 | 0.015 | 15.872 | - |
| 1000 | 0.283 | 0.025 | 11.325 | - |
| 10000 | 0.442 | 0.097 | 4.554 | - |
| 100000 | 2.546 | 0.676 | 3.766 | - |

## Thread Pool

- Benchmark id: `thread_pool`
- Note: Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 52.439 | 11.794 | 4.446 | - |
| 50 | 25.907 | 5.261 | 4.924 | - |
| 20 | 11.172 | 2.026 | 5.515 | - |
| 10 | 4.853 | 1.085 | 4.472 | - |
| 5 | 2.608 | 0.555 | 4.699 | - |

## Wavefront

- Benchmark id: `wavefront`
- Note: Report profile caps the grid size at 256x256 blocks (original default: 16384x16384).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.017 | 0.008 | 2.078 | - |
| 4 | 0.024 | 0.008 | 3.021 | - |
| 16 | 0.023 | 0.013 | 1.782 | - |
| 64 | 0.137 | 0.017 | 8.032 | - |
| 256 | 0.288 | 0.051 | 5.647 | - |
| 1024 | 1.059 | 0.143 | 7.405 | - |

