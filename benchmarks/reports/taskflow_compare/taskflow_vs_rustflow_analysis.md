# RustFlow vs Taskflow 性能分析

- 日期：2026-04-01
- 对应报告：`benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_report.md`
- 配置：`threads=4`，`rounds=1`
- 分析口径：优先看趋势，不放大小尺寸点位上的单次抖动

## 当前结论

这轮对齐和优化之后，结论已经和上一版明显不同：

1. `integrate` 基本解决。
   现在只有 `size=0` 这个纯固定开销点仍慢于 Taskflow，`100..500` 全部领先，比例大约在 `0.17x..0.23x`。
2. `skynet` 已经整体领先。
   四个测试点全部快于 Taskflow，比例大约在 `0.09x..0.82x`。
3. `nqueens` 已经从“大幅落后”收敛到“只在尾部规模落后”。
   `1..8` 大多领先，`9` 和 `10` 仍分别慢约 `1.68x` 和 `1.96x`。
4. `for_each` 和 `black_scholes` 的 benchmark 适配已经明显更接近 Taskflow 原始 workload。
   `for_each` 在 `10k` 已领先，在 `100k` 基本打平；`black_scholes` 仍慢，但差距已经压到 `3.5x..4.5x`，不再是之前那种由 adapter 放大的失真结果。

因此，当前真正还需要优先解决的，不再是“递归 benchmark 全面失守”，而是下面三类问题。

## 仍然落后的三大类

### 1. 超细粒度任务的固定调度成本仍然偏高

最典型的仍然是这些 case：

- `async_task`：17/17 点落后，最差约 `30.3x`
- `linear_chain`：16/16 点落后，最差约 `10.5x`
- `embarrassing_parallelism`：17/17 点落后，最差约 `4.0x`
- `binary_tree`：14/14 点落后，最差约 `5.2x`
- `graph_traversal`：6/7 点落后，最差约 `3.0x`
- `thread_pool`：5/5 点落后，最差约 `5.7x`

这些 workload 的共同点很明确：

- 单 task 几乎没有计算量
- benchmark 主要在测 scheduler 自身
- 固定成本比真实计算更主导总时间

前两轮已经做了 one-off async fast path，但从结果看，RustFlow 在以下路径上仍然偏重：

- async task 创建和提交
- run handle 生命周期管理
- worker 唤醒和队列交互
- 小任务下的跨线程同步

这仍然是第一优先级，因为它同时影响多个 benchmark，而不是单一 case。

### 2. 运行时递归能力已经补上，但 `fibonacci` 仍然没有过关

当前递归类 case 已经分化：

- `integrate`：已解决
- `skynet`：已反超
- `nqueens`：只剩大尺寸尾部落后
- `fibonacci`：19/20 点落后，最差约 `47.6x`

这说明“有没有运行时递归接口”已经不是主要问题，真正剩下的是 `fibonacci` 这种极端细粒度递归树上的路径成本。

`fibonacci` 的特点是：

- 节点数指数爆炸
- 每个节点几乎没有有效工作
- 运行时递归本身的调度、句柄、等待成本被放大到极致

也就是说，`fibonacci` 现在更像是“递归 tiny-task scheduler benchmark”，不是普通分治 benchmark。下一步要优化它，方向应该是：

- 继续压低 `runtime_async` / `runtime_silent_async` 的提交成本
- 减少递归等待路径里的 handle 开销
- 考虑更 work-first 的递归调度策略，而不是继续堆通用句柄语义

如果这条线不做，`fibonacci` 还会长期停留在红区。

### 3. 算法层实现仍有几组明显短板

当前报告里还有几组持续落后的算法 case：

- `scan`：5/5 点落后，最差约 `13.5x`
- `sort`：5/5 点落后，最差约 `10.8x`
- `primes`：4/5 点落后，最差约 `7.8x`
- `wavefront`：6/6 点落后，最差约 `7.9x`
- `merge_sort`：5/5 点落后，最差约 `2.0x`
- `data_pipeline`：9/10 点落后，最差约 `3.4x`
- `graph_pipeline`：4/5 点落后，最差约 `2.2x`

这类问题和前面的 scheduler 固定开销不同，更偏算法实现本身：

- `scan` / `sort` 说明当前 parallel algorithm 路径还不够轻
- `wavefront` 和 `graph_pipeline` 说明 pipeline / wavefront 组合下的框架成本还偏高
- `primes` 说明当前切块和聚合方式对轻计算 kernel 不够友好

这一组应该作为第二优先级处理，因为它们覆盖的是“算法库质量”，不是 core executor 的基本面。

## 已经验证有效的改动

这轮已经证明有效的方向有三条：

1. benchmark adapter 必须尽量和 Taskflow 原始 workload 对齐。
   `for_each`、`black_scholes` 的结果已经证明，之前一部分差距确实是 adapter 自己放大的。
2. runtime 递归必须有 worker 内联等待能力。
   没有它，`integrate` 和 `fibonacci` 这类 case 根本不在一个量级上。
3. 深层小任务必须允许顺序回退。
   `integrate` 的栈问题、`nqueens` / `skynet` 的任务风暴问题，都是靠“上层并行、深层顺序”才真正收住。

## 下一阶段建议

### P0：继续压 executor 的 tiny-task 固定成本

先盯这几个 benchmark：

- `async_task`
- `linear_chain`
- `thread_pool`
- `binary_tree`
- `embarrassing_parallelism`

这组 case 的回报最高，因为一个 runtime 优化会同时反映到多项数据上。

### P1：专项处理 `fibonacci`

目标不是再加一个新接口，而是把现有 runtime recursion 路径做轻：

- 更少的句柄对象
- 更少的等待层级
- 更偏 work-first 的递归执行

`fibonacci` 目前仍然是整份报告里最醒目的红项之一。

### P2：重做 `scan` / `sort` / `wavefront`

从报告看，这三项已经比 pipeline 更值得优先拿下：

- `scan` 和 `sort` 的倍数仍然过大
- `wavefront` 在所有点位都稳定落后

如果只做 scheduler 层优化，这三项不会自然好起来。

## 一句话判断

RustFlow 现在的主要矛盾已经从“缺少递归执行能力”转成“tiny-task 调度成本过高 + 若干算法实现还偏重”。

这意味着下一步不该再平均撒网，而应该先打掉 `async_task / fibonacci / scan / sort` 这四条主线。
