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
| 1 | 0.027 | 0.003 | 9.028 | - |
| 2 | 0.015 | 0.001 | 15.292 | - |
| 4 | 0.032 | 0.002 | 16.188 | - |
| 8 | 0.056 | 0.004 | 13.958 | - |
| 16 | 0.061 | 0.002 | 30.271 | - |
| 32 | 0.105 | 0.005 | 21.100 | - |
| 64 | 0.094 | 0.012 | 7.872 | - |
| 128 | 0.284 | 0.025 | 11.347 | - |
| 256 | 0.528 | 0.046 | 11.476 | - |
| 512 | 0.701 | 0.099 | 7.076 | - |
| 1024 | 1.280 | 0.145 | 8.827 | - |
| 2048 | 2.257 | 0.312 | 7.233 | - |
| 4096 | 5.260 | 0.622 | 8.457 | - |
| 8192 | 9.722 | 1.268 | 7.667 | - |
| 16384 | 18.199 | 2.526 | 7.205 | - |
| 32768 | 40.662 | 11.405 | 3.565 | - |
| 65536 | 76.498 | 17.777 | 4.303 | - |

## Binary Tree

- Benchmark id: `binary_tree`
- Note: Report profile caps the original Taskflow sweep at 14 layers (original default: 25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.045 | 0.021 | 2.123 | - |
| 4 | 0.038 | 0.010 | 3.804 | - |
| 8 | 0.039 | 0.018 | 2.169 | - |
| 16 | 0.031 | 0.010 | 3.108 | - |
| 32 | 0.047 | 0.009 | 5.194 | - |
| 64 | 0.093 | 0.020 | 4.640 | - |
| 128 | 0.112 | 0.032 | 3.505 | - |
| 256 | 0.130 | 0.041 | 3.180 | - |
| 512 | 0.229 | 0.090 | 2.543 | - |
| 1024 | 0.399 | 0.183 | 2.179 | - |
| 2048 | 0.818 | 0.234 | 3.494 | - |
| 4096 | 1.632 | 0.686 | 2.379 | - |
| 8192 | 2.967 | 0.854 | 3.474 | - |
| 16384 | 5.637 | 2.574 | 2.190 | - |

## Black-Scholes

- Benchmark id: `black_scholes`
- Note: Report profile caps the original Taskflow sweep at 3,000 options (original default: 10,000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1000 | 31.123 | 6.842 | 4.549 | - |
| 2000 | 71.005 | 21.529 | 3.298 | - |
| 3000 | 123.123 | 44.291 | 2.780 | - |

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.047 | 0.171 | 0.273 | - |
| 4 | 0.085 | 0.025 | 3.412 | - |
| 8 | 0.113 | 0.036 | 3.145 | - |
| 16 | 0.143 | 0.053 | 2.696 | - |
| 32 | 0.259 | 0.098 | 2.647 | - |
| 64 | 0.445 | 0.175 | 2.540 | - |
| 128 | 0.829 | 0.323 | 2.566 | - |
| 256 | 1.612 | 0.634 | 2.542 | - |
| 512 | 3.159 | 1.301 | 2.428 | - |
| 1024 | 5.976 | 2.262 | 2.642 | - |

## Embarrassing Parallelism

- Benchmark id: `embarrassing_parallelism`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^20).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.024 | 0.006 | 3.965 | - |
| 2 | 0.033 | 0.009 | 3.690 | - |
| 4 | 0.020 | 0.014 | 1.440 | - |
| 8 | 0.025 | 0.014 | 1.756 | - |
| 16 | 0.028 | 0.018 | 1.556 | - |
| 32 | 0.061 | 0.019 | 3.213 | - |
| 64 | 0.080 | 0.031 | 2.571 | - |
| 128 | 0.109 | 0.052 | 2.103 | - |
| 256 | 0.237 | 0.094 | 2.525 | - |
| 512 | 0.360 | 0.181 | 1.988 | - |
| 1024 | 0.644 | 0.350 | 1.841 | - |
| 2048 | 1.337 | 0.585 | 2.285 | - |
| 4096 | 2.833 | 1.010 | 2.805 | - |
| 8192 | 5.008 | 2.790 | 1.795 | - |
| 16384 | 11.566 | 5.402 | 2.141 | - |
| 32768 | 19.286 | 10.818 | 1.783 | - |
| 65536 | 45.406 | 21.695 | 2.093 | - |

## Fibonacci

- Benchmark id: `fibonacci`
- Note: Report profile caps the original Taskflow sweep at `fib(20)` (original default: `fib(40)`).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.015 | 0.015 | 1.019 | - |
| 2 | 0.022 | 0.008 | 2.688 | - |
| 3 | 0.031 | 0.006 | 5.146 | - |
| 4 | 0.015 | 0.016 | 0.966 | - |
| 5 | 0.022 | 0.013 | 1.696 | - |
| 6 | 0.031 | 0.009 | 3.394 | - |
| 7 | 0.069 | 0.011 | 6.269 | - |
| 8 | 0.100 | 0.024 | 4.153 | - |
| 9 | 0.088 | 0.026 | 3.377 | - |
| 10 | 0.142 | 0.037 | 3.827 | - |
| 11 | 0.182 | 0.059 | 3.079 | - |
| 12 | 0.338 | 0.083 | 4.076 | - |
| 13 | 0.486 | 0.132 | 3.685 | - |
| 14 | 0.803 | 0.195 | 4.119 | - |
| 15 | 1.106 | 0.263 | 4.205 | - |
| 16 | 1.648 | 0.057 | 28.912 | - |
| 17 | 2.702 | 0.071 | 38.061 | - |
| 18 | 4.627 | 0.105 | 44.065 | - |
| 19 | 7.469 | 0.157 | 47.572 | - |
| 20 | 11.410 | 0.250 | 45.639 | - |

## For Each

- Benchmark id: `for_each`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.027 | 0.009 | 2.958 | - |
| 100 | 0.046 | 0.020 | 2.325 | - |
| 1000 | 0.033 | 0.020 | 1.654 | - |
| 10000 | 0.044 | 0.057 | 0.766 | - |
| 100000 | 0.227 | 0.207 | 1.098 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.066 | 0.099 | 0.663 | - |
| 911 | 2.314 | 1.194 | 1.938 | - |
| 3334 | 7.635 | 3.415 | 2.236 | - |
| 7311 | 16.361 | 8.275 | 1.977 | - |
| 12904 | 28.301 | 13.070 | 2.165 | - |

## Graph Traversal

- Benchmark id: `graph_traversal`
- Note: Report profile caps the graph dimension at 64 (original default: 2048).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.024 | 0.008 | 3.037 | - |
| 12 | 0.023 | 0.085 | 0.269 | - |
| 53 | 0.054 | 0.022 | 2.458 | - |
| 219 | 0.060 | 0.040 | 1.510 | - |
| 911 | 0.161 | 0.110 | 1.466 | - |
| 3548 | 0.518 | 0.416 | 1.244 | - |
| 14187 | 1.973 | 1.589 | 1.242 | - |

## Heterogeneous Traversal

- Benchmark id: `hetero_traversal`
- Note: Skipped: Taskflow benchmark is CUDA-based and RustFlow currently has no heterogeneous execution backend.

## Integrate

- Benchmark id: `integrate`
- Note: Report profile caps the x-interval at 500 (original default: 2000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0.016 | 0.013 | 1.205 | - |
| 100 | 1.639 | 8.777 | 0.187 | - |
| 200 | 3.236 | 19.090 | 0.169 | - |
| 300 | 6.114 | 34.542 | 0.177 | - |
| 400 | 7.202 | 42.201 | 0.171 | - |
| 500 | 10.991 | 69.071 | 0.159 | - |

## Linear Chain

- Benchmark id: `linear_chain`
- Note: Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.015 | 0.004 | 3.823 | - |
| 4 | 0.026 | 0.011 | 2.333 | - |
| 8 | 0.017 | 0.011 | 1.508 | - |
| 16 | 0.013 | 0.010 | 1.304 | - |
| 32 | 0.056 | 0.009 | 6.273 | - |
| 64 | 0.081 | 0.011 | 7.322 | - |
| 128 | 0.124 | 0.016 | 7.745 | - |
| 256 | 0.171 | 0.021 | 8.125 | - |
| 512 | 0.356 | 0.038 | 9.361 | - |
| 1024 | 0.689 | 0.072 | 9.567 | - |
| 2048 | 1.373 | 0.146 | 9.404 | - |
| 4096 | 2.805 | 0.266 | 10.544 | - |
| 8192 | 5.353 | 0.520 | 10.294 | - |
| 16384 | 9.368 | 0.977 | 9.588 | - |
| 32768 | 18.714 | 1.978 | 9.461 | - |
| 65536 | 33.134 | 4.599 | 7.205 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.142 | 0.343 | 0.415 | - |
| 4 | 0.268 | 0.238 | 1.126 | - |
| 8 | 0.403 | 0.380 | 1.061 | - |
| 16 | 0.639 | 0.685 | 0.933 | - |
| 32 | 1.195 | 1.258 | 0.950 | - |
| 64 | 2.378 | 2.388 | 0.996 | - |
| 128 | 4.693 | 4.707 | 0.997 | - |
| 256 | 9.304 | 9.494 | 0.980 | - |
| 512 | 18.595 | 18.594 | 1.000 | - |
| 1024 | 37.066 | 37.212 | 0.996 | - |

## Mandelbrot

- Benchmark id: `mandelbrot`
- Note: Report profile caps the image size at 300x300 (original default: 1000x1000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 5.865 | 5.907 | 0.993 | - |
| 200 | 23.364 | 23.627 | 0.989 | - |
| 300 | 52.532 | 52.736 | 0.996 | - |

## Matrix Multiplication

- Benchmark id: `matrix_multiplication`
- Note: Report profile caps matrix size at 256 (original default: 1024).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 128 | 0.429 | 0.681 | 0.629 | - |
| 192 | 1.435 | 2.083 | 0.689 | - |
| 256 | 3.522 | 5.762 | 0.611 | - |

## Merge Sort

- Benchmark id: `merge_sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.011 | 0.011 | 1.011 | - |
| 100 | 0.018 | 0.018 | 1.002 | - |
| 1000 | 0.043 | 0.041 | 1.060 | - |
| 10000 | 0.442 | 0.267 | 1.654 | - |
| 100000 | 4.305 | 2.168 | 1.986 | - |

## MNIST

- Benchmark id: `mnist`
- Note: Skipped: Taskflow benchmark requires external Kaggle MNIST data files that are not present in this workspace.

## N-Queens

- Benchmark id: `nqueens`
- Note: Report profile caps the original Taskflow sweep at 10 queens (original default: 14).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.009 | 0.014 | 0.661 | - |
| 2 | 0.014 | 0.013 | 1.045 | - |
| 3 | 0.002 | 0.018 | 0.095 | - |
| 4 | 0.015 | 0.074 | 0.200 | - |
| 5 | 0.047 | 0.080 | 0.583 | - |
| 6 | 0.084 | 0.231 | 0.362 | - |
| 7 | 0.174 | 0.442 | 0.393 | - |
| 8 | 0.691 | 0.982 | 0.704 | - |
| 9 | 2.712 | 1.614 | 1.681 | - |
| 10 | 12.359 | 6.316 | 1.957 | - |

## Primes

- Benchmark id: `primes`
- Note: Report profile caps the original Taskflow sweep at 10^5 (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.014 | 0.015 | 0.958 | - |
| 100 | 0.019 | 0.012 | 1.549 | - |
| 1000 | 0.072 | 0.023 | 3.139 | - |
| 10000 | 0.548 | 0.089 | 6.161 | - |
| 100000 | 5.199 | 0.670 | 7.759 | - |

## Reduce Sum

- Benchmark id: `reduce_sum`
- Note: Report profile caps the original Taskflow sweep at 10^6 elements (original default: 10^9).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.028 | 0.008 | 3.516 | - |
| 100 | 0.049 | 0.011 | 4.439 | - |
| 1000 | 0.028 | 0.018 | 1.535 | - |
| 10000 | 0.144 | 0.029 | 4.976 | - |
| 100000 | 0.046 | 0.060 | 0.763 | - |
| 1000000 | 0.299 | 0.392 | 0.762 | - |

## Scan

- Benchmark id: `scan`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.051 | 0.009 | 5.630 | - |
| 100 | 0.127 | 0.013 | 9.760 | - |
| 1000 | 0.096 | 0.010 | 9.567 | - |
| 10000 | 0.183 | 0.033 | 5.539 | - |
| 100000 | 1.028 | 0.076 | 13.521 | - |

## Skynet

- Benchmark id: `skynet`
- Note: Report profile caps the original Taskflow sweep at depth 4 (original default: 8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.012 | 0.015 | 0.817 | - |
| 2 | 0.016 | 0.045 | 0.351 | - |
| 3 | 0.038 | 0.412 | 0.093 | - |
| 4 | 0.109 | 0.389 | 0.281 | - |

## Sort

- Benchmark id: `sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.086 | 0.010 | 8.617 | - |
| 100 | 0.108 | 0.010 | 10.779 | - |
| 1000 | 0.237 | 0.025 | 9.493 | - |
| 10000 | 0.301 | 0.095 | 3.165 | - |
| 100000 | 2.117 | 0.664 | 3.188 | - |

## Thread Pool

- Benchmark id: `thread_pool`
- Note: Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 51.710 | 10.243 | 5.048 | - |
| 50 | 25.238 | 5.222 | 4.833 | - |
| 20 | 10.133 | 1.901 | 5.330 | - |
| 10 | 5.234 | 1.116 | 4.690 | - |
| 5 | 2.532 | 0.445 | 5.689 | - |

## Wavefront

- Benchmark id: `wavefront`
- Note: Report profile caps the grid size at 256x256 blocks (original default: 16384x16384).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1 | 0.013 | 0.007 | 1.899 | - |
| 4 | 0.018 | 0.009 | 2.009 | - |
| 16 | 0.043 | 0.010 | 4.329 | - |
| 64 | 0.078 | 0.016 | 4.865 | - |
| 256 | 0.270 | 0.047 | 5.744 | - |
| 1024 | 0.927 | 0.117 | 7.921 | - |

