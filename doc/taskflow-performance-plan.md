# RustFlow Taskflow Performance Improvement Plan

## 背景

当前专项对比已经产出两份基线文档：

- [RustFlow vs Taskflow Benchmark Report](../benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_report.md)
- [RustFlow vs Taskflow Analysis](../benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_analysis.md)

报告说明，RustFlow 目前的主要性能短板不是数值计算内核本身，而是：

- benchmark 适配层有少数 case 不是完全等价
- one-off async task 的固定开销偏高
- 缺少轻量运行时递归执行路径
- pipeline 路径仍然偏重

这份计划的目标是把上述结论收敛成可执行的阶段路线，而不是继续停留在结论层。

## 基线结论

当前最需要关注的基线点如下：

- `async_task`：大多数点落后约 3x 到 7x
- `linear_chain`：大多数点落后约 8x 到 11x
- `binary_tree`：大多数点落后约 2x 到 5x
- `for_each`：大点位落后约 13x 到 25x
- `black_scholes`：落后约 11x 到 14x
- `fibonacci`：大点位落后约 41x 到 56x
- `integrate`：落后约 57x 到 72x
- `data_pipeline`：落后约 2.6x 到 3.1x
- `graph_pipeline`：落后约 1.8x 到 2.1x

同时也要明确两件事：

- `mandelbrot` 基本打平
- `matrix_multiplication` 当前甚至优于 Taskflow

所以接下来不应该笼统地“全面重写 runtime”，而应该按瓶颈分类逐项处理。

## 总体策略

执行顺序固定为：

1. 先修 benchmark 等价性
2. 再降低 executor 固定开销
3. 再补运行时递归能力
4. 再收紧算法层分配和 chunk 策略
5. 最后处理 pipeline 结构性开销

这样安排有两个原因：

- 先把伪差距剥离，后续报告才可信
- 先解决固定成本，能最快压低最显眼的一批红项

## 阶段 0：基线和验收方式

目标：建立固定复测口径，避免每次改动后只凭主观判断。

任务：

- 固定专项基线文档，不覆盖历史结果
- 建立一组最小复测集
- 约定每个阶段结束后的验收标准

最小复测集：

- `async_task`
- `linear_chain`
- `binary_tree`
- `for_each`
- `black_scholes`
- `fibonacci`
- `integrate`
- `data_pipeline`
- `graph_pipeline`

阶段性要求：

- 每个阶段结束后先跑最小复测集
- 阶段完成后再跑全量对比
- 报告中必须保留旧基线和新结果的可比描述

完成标准：

- 后续每一轮优化都能明确回答“哪个 case 变快了，哪个 case 没变”

## 阶段 1：修 benchmark 等价性

目标：先消除最明显的 apples-to-apples 问题。

优先 case：

- `for_each`
- `black_scholes`

涉及文件：

- `benchmarks/src/bin/taskflow_compare.rs`

具体改动：

- `for_each` 从 `parallel_transform + checksum` 改成原地并行遍历
- `black_scholes` 改成写入预分配输出 buffer
- 尽量匹配 Taskflow benchmark 的内部循环结构，而不是在 Rust 侧额外引入分配和收集
- 纯校验性质的 checksum 尽量移出计时区，或者保证两边做完全相同的校验

验收标准：

- `for_each` 和 `black_scholes` 的 ratio 明显下降
- 新报告里这两个 case 不再混入 Rust 侧额外 adapter 成本

风险：

- 如果改动过度，可能会让 Rust 版 workload 偏离 Taskflow 原 benchmark
- 必须保留与 Taskflow benchmark 语义一致的输入生成和结果校验

## 阶段 2：给 executor 增加 async fast path

目标：解决 one-off async task 不应先构造临时 `Flow` 的问题。

优先 case：

- `async_task`
- `linear_chain`
- `binary_tree`
- `embarrassing_parallelism`
- `graph_traversal`

涉及文件：

- `flow-core/src/executor.rs`
- `flow-core/src/async_task.rs`

具体改动：

- 为 `Executor::async_task` 增加 direct enqueue 路径
- 为 `Executor::silent_async` 增加 direct enqueue 路径
- 让 dependent async 内部调度复用这条轻量路径
- 保持现有的 `wait`、错误传播、panic 转换、取消语义不变

建议实现约束：

- 不为 one-off async 创建临时 `Flow`
- 不为 one-off async 创建完整 `RunState + GraphSnapshot`
- 尽量复用现有 worker queue、notifier 和 `RunHandle`

验收标准：

- `async_task` 明显改善
- `linear_chain`、`binary_tree`、`embarrassing_parallelism` 的 ratio 同步下降
- 相关测试覆盖 panic、wait、drop、依赖推进和取消行为

## 阶段 3：补运行时递归接口

目标：让分治类 workload 不再通过“先构建整棵 DAG，再整体执行”的方式运行。

优先 case：

- `fibonacci`
- `integrate`
- 次级观察：`nqueens`、`skynet`

涉及文件：

- `flow-core/src/runtime.rs`
- `flow-core/src/executor.rs`
- `benchmarks/src/bin/taskflow_compare.rs`

具体改动：

- 设计一个轻量级 runtime recursion API，思路接近 `task_group + corun`
- 允许 worker 在等待子任务时继续工作
- 避免用高频 `Arc<Mutex<Option<T>>>` 作为递归结果合并主路径

验收标准：

- `fibonacci` 和 `integrate` 的 ratio 从数量级落后降到同一数量级内竞争
- 不引入 worker 饥饿、死锁或 run handle 提前完成问题

风险：

- 这是运行时语义级改动，回归面明显大于阶段 1 和阶段 2
- 需要补足递归调度的单元测试和 benchmark 级验证

## 阶段 4：瘦身算法层基础设施

目标：减少 `parallel_transform`、`scan`、`sort` 等算法实现中的额外分配和拆分成本。

优先 case：

- `for_each`
- `black_scholes`
- `scan`
- `sort`
- `reduce_sum` 小规模点

涉及文件：

- `flow-algorithms/src/reduce_transform.rs`
- `flow-algorithms/src/parallel_for.rs`
- `flow-algorithms/src/find_scan_sort.rs`

具体改动：

- 增加 `parallel_transform_into`，直接写入预分配输出
- 去掉每个输出元素一个 `Mutex<Option<T>>` 的路径
- `Auto` chunk 计算基于 `executor.num_workers()`，不是宿主机总并发度
- 给 `parallel_sort` 增加小规模 cutoff，减少过度任务拆分和 merge 轮次
- 对 `scan` 先做 profile，再决定是 chunk prefix 路径还是 `async_task` 开销主导

验收标准：

- `for_each`、`black_scholes` 进一步改善
- `scan` 和 `sort` 的小规模点明显下降
- `mandelbrot` 和 `matrix_multiplication` 不出现回退

## 阶段 5：优化 pipeline

目标：处理 pipeline 的真实结构性成本，而不是把所有性能问题都归因到 executor。

优先 case：

- `data_pipeline`
- `graph_pipeline`
- 次级观察：`linear_pipeline`

涉及文件：

- `flow-core/src/pipeline.rs`

具体改动：

- 去掉 `Pipeline::run` / `DataPipeline::run` 的额外协调线程
- 优先保留静态 typed pipeline 路径
- 尽量绕开 `Box<dyn Any + Send>` 的 payload 热路径
- 降低 serial stage 的锁与 bookkeeping 成本

验收标准：

- `data_pipeline` 明显改善
- `graph_pipeline` 明显改善
- `linear_pipeline` 的 steady-state 不退化

风险：

- pipeline 改动有 API 和实现双重影响
- 如果过早大改，容易和前面的 runtime 改动混在一起，导致收益归因不清

## 复测和汇报节奏

建议固定如下节奏：

1. 每完成一个阶段，先跑最小复测集
2. 如果收益符合预期，再跑全量报告
3. 每个阶段都产出一段简短结论：
   - 哪些 case 改善了
   - 哪些 case 没改善
   - 是否有回退
4. 全量报告更新后，再决定下一阶段是否需要调整优先级

## 推荐排期

如果只安排一个工程师连续推进，可以按下面的节奏：

- 第 1 周：阶段 1 + 阶段 2
- 第 2 周：阶段 3
- 第 3 周：阶段 4
- 第 4 周：阶段 5 + 全量复测 + 报告更新

如果阶段 2 收益已经足够显著，可以在第 2 周开始时重新评估阶段 3 和阶段 4 的先后顺序。

## 跟踪清单

- [ ] 冻结当前基线报告并确定最小复测集
- [ ] 调整 `for_each` benchmark 结构
- [ ] 调整 `black_scholes` benchmark 结构
- [ ] 给 `Executor::async_task` 增加 fast path
- [ ] 给 `Executor::silent_async` 增加 fast path
- [ ] 让 dependent async 复用轻量调度路径
- [ ] 设计并实现 runtime recursion API
- [ ] 用 runtime recursion 重写 `fibonacci` benchmark 路径
- [ ] 用 runtime recursion 重写 `integrate` benchmark 路径
- [ ] 增加 `parallel_transform_into`
- [ ] 调整 auto chunk 策略
- [ ] 优化 `parallel_sort`
- [ ] profile 并优化 `scan`
- [ ] 去掉 pipeline 额外协调线程
- [ ] 收紧 `DataPipeline` payload 热路径
- [ ] 阶段性复测并更新专项报告

## 成功标准

这轮优化完成后，至少应达到以下结果：

- `async_task`、`linear_chain` 相比当前基线显著收敛
- `for_each`、`black_scholes` 的对比具备可信 benchmark 等价性
- `fibonacci`、`integrate` 不再出现数量级级别的巨大差距
- `data_pipeline`、`graph_pipeline` 的 ratio 明显下降
- 现有优势项如 `mandelbrot`、`matrix_multiplication` 不回退

如果这些标准达不到，就不应该急着继续扩展 API，而应该先回到 profiling 和实现细节层面复盘。
