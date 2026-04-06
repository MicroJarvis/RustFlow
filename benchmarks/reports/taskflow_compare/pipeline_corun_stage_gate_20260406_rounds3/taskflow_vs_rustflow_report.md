# RustFlow vs Taskflow Benchmark Report

- Date: 2026-03-31
- Profile: Report
- Threads: 4
- Rounds per point: 3
- Ratio column: RustFlow ms / Taskflow ms (smaller is better)
- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution.

## Data Pipeline

- Benchmark id: `data_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.046 | 0.029 | 1.581 | - |
| 4 | 0.062 | 0.018 | 3.520 | - |
| 8 | 0.104 | 0.028 | 3.710 | - |
| 16 | 0.178 | 0.041 | 4.369 | - |
| 32 | 0.302 | 0.135 | 2.233 | - |
| 64 | 0.617 | 0.264 | 2.338 | - |
| 128 | 1.111 | 0.372 | 2.986 | - |
| 256 | 2.097 | 0.680 | 3.083 | - |
| 512 | 3.755 | 1.301 | 2.887 | - |
| 1024 | 6.551 | 2.136 | 3.067 | - |

## Graph Pipeline

- Benchmark id: `graph_pipeline`
- Note: Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.027 | 0.030 | 0.910 | - |
| 911 | 2.103 | 0.890 | 2.363 | - |
| 3334 | 7.454 | 2.688 | 2.773 | - |
| 7311 | 16.681 | 5.825 | 2.864 | - |
| 12904 | 29.109 | 10.423 | 2.793 | - |

## Linear Pipeline

- Benchmark id: `linear_pipeline`
- Note: Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).

| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |
| ---: | ---: | ---: | ---: | --- |
| 2 | 0.158 | 0.153 | 1.030 | - |
| 4 | 0.283 | 0.202 | 1.399 | - |
| 8 | 0.474 | 0.332 | 1.426 | - |
| 16 | 1.036 | 0.577 | 1.796 | - |
| 32 | 2.019 | 1.049 | 1.926 | - |
| 64 | 4.071 | 2.092 | 1.946 | - |
| 128 | 8.115 | 4.225 | 1.921 | - |
| 256 | 16.308 | 8.338 | 1.956 | - |
| 512 | 32.457 | 16.533 | 1.963 | - |
| 1024 | 64.786 | 33.081 | 1.958 | - |

