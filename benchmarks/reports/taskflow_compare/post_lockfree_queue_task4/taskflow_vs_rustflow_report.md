# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Black-Scholes

- Benchmark id: `black_scholes`
- Note: Report profile caps the original Taskflow sweep at 3,000 options (original default: 10,000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 1000 | 20.344 | 8.351 | 2.436 | - |
| 2000 | 50.639 | 26.983 | 1.877 | - |
| 3000 | 89.407 | 55.790 | 1.603 | - |

## For Each

- Benchmark id: `for_each`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.017 | 0.009 | 1.877 | - |
| 100 | 0.021 | 0.009 | 2.196 | - |
| 1000 | 0.016 | 0.014 | 1.199 | - |
| 10000 | 0.036 | 0.038 | 0.944 | - |
| 100000 | 0.177 | 0.144 | 1.226 | - |

## Mandelbrot

- Benchmark id: `mandelbrot`
- Note: Report profile caps the image size at 300x300 (original default: 1000x1000).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 100 | 9.761 | 4.688 | 2.082 | - |
| 200 | 22.998 | 17.679 | 1.301 | - |
| 300 | 47.736 | 39.580 | 1.206 | - |

## Matrix Multiplication

- Benchmark id: `matrix_multiplication`
- Note: Report profile caps matrix size at 256 (original default: 1024).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 128 | 0.406 | 0.466 | 0.871 | - |
| 192 | 1.118 | 1.647 | 0.679 | - |
| 256 | 2.607 | 4.266 | 0.611 | - |

## Merge Sort

- Benchmark id: `merge_sort`
- Note: Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 10 | 0.006 | 0.013 | 0.420 | - |
| 100 | 0.006 | 0.016 | 0.395 | - |
| 1000 | 0.022 | 0.032 | 0.682 | - |
| 10000 | 0.299 | 0.148 | 2.019 | - |
| 100000 | 3.859 | 1.674 | 2.305 | - |

