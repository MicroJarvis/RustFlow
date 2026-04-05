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
| 1 | 0.009 | 0.002 | 5.583 | - |
| 2 | 0.006 | 0.002 | 2.806 | - |
| 4 | 0.006 | 0.002 | 3.775 | - |
| 8 | 0.007 | 0.003 | 2.583 | - |
| 16 | 0.011 | 0.004 | 2.567 | - |
| 32 | 0.010 | 0.010 | 0.981 | - |
| 64 | 0.022 | 0.025 | 0.905 | - |
| 128 | 0.082 | 0.038 | 2.129 | - |
| 256 | 0.140 | 0.072 | 1.932 | - |
| 512 | 0.223 | 0.065 | 3.428 | - |
| 1024 | 0.350 | 0.185 | 1.890 | - |
| 2048 | 0.694 | 0.280 | 2.483 | - |
| 4096 | 1.319 | 0.664 | 1.986 | - |
| 8192 | 2.537 | 2.597 | 0.977 | - |
| 16384 | 5.136 | 5.181 | 0.991 | - |
| 32768 | 9.500 | 8.557 | 1.110 | - |
| 65536 | 18.361 | 18.893 | 0.972 | - |

## Binary Tree

- Benchmark id: `binary_tree`
- Note: Report profile caps the original Taskflow sweep at 14 layers (original default: 25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.020 | 0.009 | 2.218 | - |
| 4 | 0.008 | 0.012 | 0.717 | - |
| 8 | 0.010 | 0.008 | 1.233 | - |
| 16 | 0.014 | 0.022 | 0.637 | - |
| 32 | 0.012 | 0.014 | 0.872 | - |
| 64 | 0.019 | 0.019 | 1.004 | - |
| 128 | 0.054 | 0.020 | 2.715 | - |
| 256 | 0.139 | 0.055 | 2.516 | - |
| 512 | 0.260 | 0.072 | 3.588 | - |
| 1024 | 0.483 | 0.124 | 3.882 | - |
| 2048 | 0.908 | 0.316 | 2.874 | - |
| 4096 | 1.704 | 0.542 | 3.145 | - |
| 8192 | 3.419 | 0.986 | 3.466 | - |
| 16384 | 6.925 | 1.978 | 3.502 | - |

## Black-Scholes

- Benchmark id: `black_scholes`
- Note: Report profile caps the original Taskflow sweep at 3,000 options (original default: 10,000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1000 | 24.371 | 9.014 | 2.704 | - |
| 2000 | 59.347 | 28.989 | 2.047 | - |
| 3000 | 104.516 | 62.529 | 1.671 | - |

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.039 | 0.046 | 0.851 | - |
| 4 | 0.054 | 0.035 | 1.565 | - |
| 8 | 0.105 | 0.042 | 2.499 | - |
| 16 | 0.198 | 0.054 | 3.675 | - |
| 32 | 0.381 | 0.096 | 3.968 | - |
| 64 | 0.756 | 0.166 | 4.552 | - |
| 128 | 1.477 | 0.276 | 5.350 | - |
| 256 | 2.966 | 0.533 | 5.569 | - |
| 512 | 5.849 | 1.048 | 5.579 | - |
| 1024 | 11.633 | 2.089 | 5.570 | - |

## Embarrassing Parallelism

- Benchmark id: `embarrassing_parallelism`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^20).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.020 | 0.008 | 2.413 | - |
| 2 | 0.015 | 0.014 | 1.093 | - |
| 4 | 0.019 | 0.017 | 1.069 | - |
| 8 | 0.017 | 0.016 | 1.054 | - |
| 16 | 0.025 | 0.020 | 1.266 | - |
| 32 | 0.030 | 0.022 | 1.367 | - |
| 64 | 0.054 | 0.028 | 1.925 | - |
| 128 | 0.081 | 0.047 | 1.746 | - |
| 256 | 0.146 | 0.079 | 1.843 | - |
| 512 | 0.279 | 0.146 | 1.916 | - |
| 1024 | 0.520 | 0.275 | 1.890 | - |
| 2048 | 1.007 | 0.524 | 1.923 | - |
| 4096 | 1.933 | 1.011 | 1.913 | - |
| 8192 | 3.621 | 2.049 | 1.767 | - |
| 16384 | 7.344 | 3.934 | 1.867 | - |
| 32768 | 14.737 | 8.046 | 1.832 | - |
| 65536 | 28.664 | 16.356 | 1.753 | - |

## Fibonacci

- Benchmark id: `fibonacci`
- Note: Report profile caps the original Taskflow sweep at `fib(20)` (original default: `fib(40)`).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.016 | 0.019 | 0.802 | - |
| 2 | 0.013 | 0.011 | 1.260 | - |
| 3 | 0.011 | 0.011 | 1.004 | - |
| 4 | 0.014 | 0.010 | 1.376 | - |
| 5 | 0.009 | 0.012 | 0.770 | - |
| 6 | 0.019 | 0.015 | 1.299 | - |
| 7 | 0.031 | 0.015 | 2.021 | - |
| 8 | 0.030 | 0.018 | 1.640 | - |
| 9 | 0.036 | 0.027 | 1.351 | - |
| 10 | 0.053 | 0.009 | 5.679 | - |
| 11 | 0.076 | 0.042 | 1.803 | - |
| 12 | 0.111 | 0.072 | 1.531 | - |
| 13 | 0.166 | 0.097 | 1.707 | - |
| 14 | 0.246 | 0.066 | 3.753 | - |
| 15 | 0.337 | 0.055 | 6.083 | - |
| 16 | 0.554 | 0.083 | 6.675 | - |
| 17 | 0.876 | 0.110 | 7.961 | - |
| 18 | 1.403 | 0.132 | 10.625 | - |
| 19 | 1.939 | 0.206 | 9.412 | - |
| 20 | 3.561 | 0.288 | 12.363 | - |

## For Each

- Benchmark id: `for_each`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.014 | 0.014 | 1.023 | - |
| 100 | 0.013 | 0.020 | 0.655 | - |
| 1000 | 0.026 | 0.020 | 1.332 | - |
| 10000 | 0.036 | 0.041 | 0.876 | - |
| 100000 | 0.199 | 0.177 | 1.124 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.029 | 0.054 | 0.536 | - |
| 911 | 4.177 | 1.280 | 3.264 | - |
| 3334 | 15.591 | 4.078 | 3.823 | - |
| 7311 | 34.461 | 8.699 | 3.961 | - |
| 12904 | 60.266 | 15.797 | 3.815 | - |

## Graph Traversal

- Benchmark id: `graph_traversal`
- Note: Report profile caps the graph dimension at 64 (original default: 2048).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.015 | 0.014 | 1.109 | - |
| 12 | 0.012 | 0.015 | 0.811 | - |
| 53 | 0.009 | 0.022 | 0.433 | - |
| 219 | 0.023 | 0.038 | 0.615 | - |
| 911 | 0.077 | 0.090 | 0.858 | - |
| 3548 | 0.204 | 0.294 | 0.692 | - |
| 14187 | 0.787 | 1.086 | 0.724 | - |

## Heterogeneous Traversal

- Benchmark id: `hetero_traversal`
- Note: Skipped: Taskflow benchmark is CUDA-based and RustFlow currently has no heterogeneous execution backend.

## Integrate

- Benchmark id: `integrate`
- Note: Report profile caps the x-interval at 500 (original default: 2000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0.019 | 0.018 | 1.061 | - |
| 100 | 0.962 | 7.625 | 0.126 | - |
| 200 | 1.993 | 15.690 | 0.127 | - |
| 300 | 3.966 | 29.291 | 0.135 | - |
| 400 | 4.046 | 34.666 | 0.117 | - |
| 500 | 7.725 | 57.686 | 0.134 | - |

## Linear Chain

- Benchmark id: `linear_chain`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.011 | 0.010 | 1.067 | - |
| 4 | 0.017 | 0.008 | 2.156 | - |
| 8 | 0.014 | 0.012 | 1.156 | - |
| 16 | 0.019 | 0.011 | 1.746 | - |
| 32 | 0.017 | 0.017 | 1.011 | - |
| 64 | 0.023 | 0.017 | 1.354 | - |
| 128 | 0.026 | 0.023 | 1.124 | - |
| 256 | 0.043 | 0.032 | 1.363 | - |
| 512 | 0.073 | 0.062 | 1.183 | - |
| 1024 | 0.134 | 0.072 | 1.864 | - |
| 2048 | 0.252 | 0.126 | 2.001 | - |
| 4096 | 0.477 | 0.189 | 2.522 | - |
| 8192 | 0.913 | 0.321 | 2.842 | - |
| 16384 | 1.931 | 0.597 | 3.233 | - |
| 32768 | 4.235 | 1.210 | 3.501 | - |
| 65536 | 8.460 | 3.141 | 2.694 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.244 | 0.233 | 1.047 | - |
| 4 | 0.500 | 0.199 | 2.510 | - |
| 8 | 0.974 | 0.338 | 2.880 | - |
| 16 | 1.910 | 0.558 | 3.424 | - |
| 32 | 3.815 | 1.016 | 3.756 | - |
| 64 | 7.703 | 2.066 | 3.729 | - |
| 128 | 15.248 | 4.010 | 3.802 | - |
| 256 | 30.466 | 8.343 | 3.652 | - |
| 512 | 60.978 | 15.820 | 3.854 | - |
| 1024 | 122.170 | 32.239 | 3.790 | - |

## Mandelbrot

- Benchmark id: `mandelbrot`
- Note: Report profile caps the image size at 300x300 (original default: 1000x1000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 15.406 | 5.560 | 2.771 | - |
| 200 | 61.316 | 19.460 | 3.151 | - |
| 300 | 140.524 | 42.435 | 3.312 | - |

## Matrix Multiplication

- Benchmark id: `matrix_multiplication`
- Note: Report profile caps matrix size at 256 (original default: 1024).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 128 | 0.913 | 0.786 | 1.162 | - |
| 192 | 3.295 | 1.943 | 1.696 | - |
| 256 | 8.043 | 4.619 | 1.741 | - |

## Merge Sort

- Benchmark id: `merge_sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.012 | 0.026 | 0.472 | - |
| 100 | 0.009 | 0.020 | 0.479 | - |
| 1000 | 0.024 | 0.044 | 0.547 | - |
| 10000 | 0.301 | 0.189 | 1.598 | - |
| 100000 | 3.124 | 1.829 | 1.708 | - |

## MNIST

- Benchmark id: `mnist`
- Note: Skipped: Taskflow benchmark requires external Kaggle MNIST data files that are not present in this workspace.

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.010 | 0.020 | 0.501 | - |
| 2 | 0.009 | 0.018 | 0.492 | - |
| 3 | 0.004 | 0.022 | 0.167 | - |
| 4 | 0.005 | 0.025 | 0.187 | - |
| 5 | 0.005 | 0.055 | 0.092 | - |
| 6 | 0.013 | 0.138 | 0.093 | - |
| 7 | 0.052 | 0.251 | 0.206 | - |
| 8 | 0.270 | 0.431 | 0.625 | - |
| 9 | 1.430 | 1.156 | 1.237 | - |
| 10 | 7.701 | 4.548 | 1.694 | - |

## Primes

- Benchmark id: `primes`
- Note: Report profile caps the original Taskflow sweep at 10^5 (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.010 | 0.010 | 0.922 | - |
| 100 | 0.010 | 0.009 | 1.109 | - |
| 1000 | 0.016 | 0.026 | 0.605 | - |
| 10000 | 0.034 | 0.061 | 0.564 | - |
| 100000 | 0.330 | 0.534 | 0.618 | - |

## Reduce Sum

- Benchmark id: `reduce_sum`
- Note: Report profile caps the original Taskflow sweep at 10^6 elements (original default: 10^9).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.017 | 0.005 | - |
| 100 | 0.000 | 0.022 | 0.002 | - |
| 1000 | 0.000 | 0.016 | 0.027 | - |
| 10000 | 0.005 | 0.018 | 0.302 | - |
| 100000 | 0.037 | 0.043 | 0.876 | - |
| 1000000 | 0.203 | 0.218 | 0.934 | - |

## Scan

- Benchmark id: `scan`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.018 | 0.017 | - |
| 100 | 0.001 | 0.019 | 0.040 | - |
| 1000 | 0.007 | 0.018 | 0.373 | - |
| 10000 | 0.061 | 0.027 | 2.269 | - |
| 100000 | 0.598 | 0.095 | 6.312 | - |

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.009 | 0.015 | 0.642 | - |
| 2 | 0.004 | 0.038 | 0.108 | - |
| 3 | 0.006 | 0.198 | 0.030 | - |
| 4 | 0.024 | 0.198 | 0.118 | - |

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.000 | 0.018 | 0.013 | - |
| 100 | 0.001 | 0.017 | 0.079 | - |
| 1000 | 0.011 | 0.029 | 0.384 | - |
| 10000 | 0.124 | 0.101 | 1.228 | - |
| 100000 | 2.009 | 0.639 | 3.145 | - |

## Thread Pool

- Benchmark id: `thread_pool`
- Note: Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 39.291 | 6.919 | 5.678 | - |
| 50 | 19.275 | 3.335 | 5.780 | - |
| 20 | 7.867 | 1.271 | 6.188 | - |
| 10 | 3.930 | 0.654 | 6.009 | - |
| 5 | 1.972 | 0.278 | 7.093 | - |

## Wavefront

- Benchmark id: `wavefront`
- Note: Report profile caps the grid size at 256x256 blocks (original default: 16384x16384).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.011 | 0.007 | 1.449 | - |
| 4 | 0.012 | 0.011 | 1.103 | - |
| 16 | 0.027 | 0.014 | 1.995 | - |
| 64 | 0.053 | 0.019 | 2.768 | - |
| 256 | 0.170 | 0.029 | 5.810 | - |
| 1024 | 0.581 | 0.144 | 4.034 | - |

