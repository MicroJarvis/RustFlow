# RustFlow Taskflow Performance Improvement Plan

## 背景

当前专项对比已经产出两份基线文档：

- [RustFlow vs Taskflow Benchmark Report](../benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_report.md)
- [RustFlow vs Taskflow Analysis](../benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_analysis.md)

截至 2026-04-01，这份计划不再按最早版本的“阶段 1 到阶段 5 原样推进”，而是基于最新报告和当前代码状态重新整理：

- 一部分原计划工作已经落地
- 当前主要矛盾已经发生变化
- 接下来更重要的是按收益排序继续压关键红项

这份文档的目标，是把“当前还需要做什么”写成和仓库现状一致的开发路线，而不是保留已经过时的待办列表。

## 当前判断

当前的总体判断仍然成立：

- 不需要笼统地“全面重写 runtime”
- 需要按瓶颈类型逐项处理
- 先打掉固定成本最高、收益最广的路径
- 再处理算法层和 pipeline 的结构性开销

但和最早一版计划相比，今天的重点已经不是：

- benchmark 适配普遍不等价
- 缺少 runtime recursion 能力

而是：

- tiny-task 调度固定成本仍然偏高
- `fibonacci` 这类极端细粒度递归树仍然很重
- `scan` / `sort` / `primes` / `wavefront` / pipeline 等算法层实现仍偏重

## 已完成的工作

下面这些方向已经不是“待设计”，而是已经在代码中落地并被最新报告验证过：

### 1. benchmark adapter 对齐

- `for_each` 已改成原地并行遍历
- `black_scholes` 已改成写入预分配输出 buffer
- 这两项的结果已经比旧报告更接近 Taskflow 原始 workload

### 2. one-off async fast path

- `Executor::async_task`
- `Executor::silent_async`
- dependent async 的内部调度

这些路径都已经直接进入 executor queue，不再先为 one-off async 构造临时 `Flow`。

### 3. runtime recursion 基础能力

- `RuntimeCtx` 已经具备 `corun` / `corun_handle` / `corun_handles`
- worker 等待子任务时已经可以继续工作
- benchmark 中的 `fibonacci` / `integrate` / `nqueens` / `skynet` 已经切到 runtime recursion 路径

### 4. 已验证有效的策略

当前结果已经证明下面几条方向是对的：

- benchmark adapter 要尽量与 Taskflow 原 benchmark 对齐
- runtime recursion 必须支持 worker 内联等待
- 深层递归必须允许顺序回退，避免任务风暴和栈问题

## 最新基线结论

截至最新分析，现状可以概括为：

### 已明显收敛或已领先

- `integrate`：基本解决
- `skynet`：整体领先
- `nqueens`：只在大尺寸尾部落后
- `for_each`：大点位已接近打平
- `mandelbrot`：基本打平
- `matrix_multiplication`：当前领先

### 仍然最值得优先处理

- `async_task`
- `linear_chain`
- `binary_tree`
- `embarrassing_parallelism`
- `thread_pool`
- `fibonacci`
- `scan`
- `sort`
- `primes`
- `wavefront`
- `data_pipeline`
- `graph_pipeline`

## 开发主线

接下来建议固定按下面四条主线推进，而不是回到旧版阶段顺序。

### P0：继续压 executor 的 tiny-task 固定成本

目标：降低提交、完成、等待、唤醒这些纯调度成本。

优先 case：

- `async_task`
- `linear_chain`
- `binary_tree`
- `embarrassing_parallelism`
- `thread_pool`
- 次级观察：`graph_traversal`

主要文件：

- `flow-core/src/executor.rs`
- `flow-core/src/async_handle.rs`
- `flow-core/src/async_task.rs`

优先改动：

- 降低 `RunHandle` 完成态查询和等待路径的锁成本
- 降低 `active_runs` 计数与 `wait_for_all` 的同步成本
- 降低 async task 提交后的唤醒和队列交互成本
- 尽量让 runtime 内部提交优先落到当前 worker，减少跨线程同步
- dependent async 继续复用最轻的提交路径

实现约束：

- 不能破坏现有 `wait`、错误传播、panic 转换、取消语义
- 不能引入 run handle 提前完成
- 不能牺牲当前 DAG 执行正确性来换 benchmark 数据

验收标准：

- `async_task` 明显改善
- `linear_chain`、`binary_tree`、`embarrassing_parallelism` 同步下降
- 相关单测覆盖 `wait`、panic、取消、依赖推进

### P1：专项处理 `fibonacci`

目标：不是再加新递归接口，而是把现有 runtime recursion 路径做轻。

优先 case：

- `fibonacci`
- 次级观察：`nqueens`

主要文件：

- `flow-core/src/executor.rs`
- `flow-core/src/runtime.rs`
- `flow-core/src/async_handle.rs`
- `benchmarks/src/bin/taskflow_compare.rs`

优先改动：

- 继续压低 `runtime_async` / `runtime_silent_async` 的提交成本
- 减少递归等待路径上的 handle 对象和等待层级
- 偏向 work-first，而不是每层都走完整通用句柄语义
- 在不偏离 Taskflow benchmark 形状的前提下，保留深层顺序回退能力

验收标准：

- `fibonacci` 明显下降
- 不引入 worker 饥饿、死锁或递归等待卡死

### P2：瘦身算法层基础设施

目标：减少 `parallel_transform`、`scan`、`sort`、轻量算法 case 中的额外分配和拼接成本。

优先 case：

- `scan`
- `sort`
- `primes`
- `reduce_sum` 小规模点
- 次级观察：`for_each`、`black_scholes`

主要文件：

- `flow-algorithms/src/reduce_transform.rs`
- `flow-algorithms/src/find_scan_sort.rs`
- `flow-algorithms/src/parallel_for.rs`

优先改动：

- 增加 `parallel_transform_into`，直接写入预分配输出
- 去掉“每个输出元素一个 `Mutex<Option<T>>`”的路径
- 去掉 `parallel_transform` / `parallel_reduce` 中 run 后额外自旋等待的路径
- 让 `Auto` chunk 策略和 executor worker 数保持一致
- 给 `parallel_sort` 增加小规模 cutoff，减少过度拆分和 merge 轮次
- 让 `scan` 优先走预分配写回，而不是 chunk 结果 `Vec` 再 flatten

验收标准：

- `scan` 和 `sort` 的倍数明显下降
- `primes` 和 `reduce_sum` 小规模点同步改善
- `mandelbrot`、`matrix_multiplication` 不出现回退

### P3：优化 pipeline

目标：处理 pipeline 的真实结构性成本。

优先 case：

- `data_pipeline`
- `graph_pipeline`
- 次级观察：`linear_pipeline`、`wavefront`

主要文件：

- `flow-core/src/pipeline.rs`

优先改动：

- 去掉 `Pipeline::run` / `DataPipeline::run` 的额外协调线程
- 优先保留静态 typed pipeline 路径
- 降低 serial stage 的锁和 bookkeeping 成本
- 在必要时再处理 `Box<dyn Any + Send>` 的 payload 热路径

实现约束：

- `linear_pipeline` 的 steady-state 不退化
- API 变动要谨慎，优先先做内部降重

验收标准：

- `data_pipeline` 明显改善
- `graph_pipeline` 明显改善
- `linear_pipeline` 不回退

## 推荐推进顺序

如果按单人连续推进，建议采用下面的顺序：

1. 先做 P0，把 executor tiny-task 成本再压一轮
2. 再做 P1，把 `fibonacci` 从现有 runtime recursion 路径继续做轻
3. 然后做 P2，优先拿下 `scan` / `sort`
4. 最后做 P3，处理 pipeline 的结构性成本

原因：

- P0 会同时影响多项 benchmark，回报最高
- `fibonacci` 当前仍是最醒目的红项之一，值得单独看
- `scan` / `sort` / `primes` 的问题不会靠 scheduler 优化自动消失
- pipeline 改动面更广，放后面更容易归因

## 复测和汇报节奏

建议固定如下节奏：

1. 每完成一条主线中的一个子阶段，先跑最小复测集
2. 如果收益符合预期，再跑全量报告
3. 每次优化后都记录三件事：
   - 哪些 case 变快了
   - 哪些 case 没变
   - 是否有回退
4. 每一轮报告都保留旧基线和新结果的可比描述

当前建议的最小复测集：

- `async_task`
- `linear_chain`
- `binary_tree`
- `fibonacci`
- `scan`
- `sort`
- `data_pipeline`
- `graph_pipeline`

如果本轮改动命中算法层，可额外补跑：

- `primes`
- `reduce_sum`
- `for_each`
- `black_scholes`

## 当前跟踪清单

### 已完成

- [x] 调整 `for_each` benchmark 结构
- [x] 调整 `black_scholes` benchmark 结构
- [x] 给 `Executor::async_task` 增加 direct enqueue 路径
- [x] 给 `Executor::silent_async` 增加 direct enqueue 路径
- [x] 让 dependent async 复用轻量调度路径
- [x] 设计并实现 runtime recursion 基础接口
- [x] 用 runtime recursion 重写 `fibonacci` benchmark 路径
- [x] 用 runtime recursion 重写 `integrate` benchmark 路径

### 进行中

- [ ] 继续压 executor tiny-task 固定成本
- [ ] 降低 runtime recursion 等待路径的 handle 成本
- [ ] 为 `parallel_transform` 增加预分配输出写回路径
- [ ] 去掉算法层结果聚合中的多余锁和自旋等待
- [ ] 优化 `parallel_sort` 的 cutoff 和 merge 策略
- [ ] 重做 `scan` 的 chunk 输出路径
- [ ] 去掉 pipeline 额外协调线程
- [ ] 视复测结果决定是否继续收紧 `DataPipeline` payload 热路径
- [ ] 阶段性复测并更新专项报告

## 成功标准

这轮继续优化后，至少应达到以下结果：

- `async_task`、`linear_chain`、`binary_tree` 相比当前基线继续收敛
- `fibonacci` 不再长期停留在最醒目的数量级红区
- `scan`、`sort` 的倍数明显下降
- `data_pipeline`、`graph_pipeline` 的 ratio 明显下降
- 已有优势项如 `integrate`、`skynet`、`matrix_multiplication` 不回退

如果这些标准达不到，就不应该急着继续扩展 API，而应该先回到 profiling 和实现细节层面复盘。
