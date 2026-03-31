# RustFlow vs Taskflow 性能分析

- 日期：2026-04-01
- 对应报告：`benchmarks/reports/taskflow_compare/taskflow_vs_rustflow_report.md`
- 分析范围：仅分析报告中 `RustFlow` 慢于 `Taskflow` 的测试用例

## 总结

当前报告里 RustFlow 落后的用例，不是由单一原因造成的，主要可以归纳为四类：

1. RustFlow 在超细粒度任务场景下，固定调度开销偏高。
2. RustFlow 目前缺少轻量级的运行时递归执行路径，导致分治类任务表现较差。
3. RustFlow 当前的 pipeline 实现比 Taskflow 更重。
4. 少数 Rust 侧 benchmark 适配并不完全等价于原始 Taskflow benchmark，因此把额外的分配和同步成本也计入了测试结果。

这份报告同时也给了一个重要的反向信号：RustFlow 并不是在所有 CPU 并行计算任务上都慢于 Taskflow。`mandelbrot` 基本打平，`matrix_multiplication` 在当前报告里甚至更快。这说明主要问题不在于“Rust 做数值计算天然更慢”，而在于细粒度调度路径和 pipeline 路径目前还不够轻。

## 整体结论

### 1. 超小任务暴露了 RustFlow 的固定开销

最明显的模式出现在 `async_task`、`embarrassing_parallelism`、`binary_tree`、`linear_chain` 和 `graph_traversal` 这些用例上。

这些 workload 的特点是：

- 单个 task 的有效工作量很小
- benchmark 更像是在测调度器本身，而不是测计算核

在这种场景下，真正主导总耗时的往往不是 task body，而是每个 task 的控制成本，例如：

- task 包装和闭包管理
- 依赖关系记录
- run handle 创建
- 图快照构建
- worker 唤醒和工作入队

RustFlow 在这类固定成本上的负担目前明显高于 Taskflow。

尤其是 `Executor::async_task` 和 `Executor::silent_async`，并不是直接把一个 runnable task 入 executor 队列，而是先临时创建一个单节点 `Flow`，然后再走完整的 flow 执行路径。也就是说，即便只是一个一次性的异步任务，当前也要额外支付：

- `Flow::new`
- 向 flow 图中插入 task
- graph snapshot 构建
- run state 初始化
- root task 调度

这条路径的直接结果就是 `async_task` 很差，同时也会拖慢一切以小 task 为主的 benchmark。

### 2. 分治递归场景是“整棵 DAG 先建完再执行”

`fibonacci` 和 `integrate` 是当前差距最大的两类用例。这里的问题不是一句“Taskflow 更优化”就能概括，而是两边使用了不同的执行模型。

Taskflow 使用的是运行时递归：

- 一边分支异步提交
- 另一边分支由当前 worker 继续计算
- 最后通过 `corun` 避免 worker 在等待子任务时空转

RustFlow 当前采用的是另一种模式：

- 先递归构建完整的 `Flow`
- 把整棵递归树全部表示成 DAG
- 然后再统一交给 executor 执行

这会让 RustFlow 在一个 benchmark 中同时承担三类成本：

- 递归建图成本
- 每个节点的依赖和 handle 成本
- 中间结果存储与汇总成本

当前 `fibonacci` 和 `integrate` 的 Rust 版本里，还为每个结果节点分配了 `Arc<Mutex<Option<T>>>` 类型的 slot，用于在父节点中合并左右子树结果。这样做逻辑上没有问题，但对这种高扇出、高节点数的递归 workload 来说会明显重于 Taskflow 的运行时递归路径。

因此，`fibonacci` 和 `integrate` 更适合被理解为：RustFlow 当前还没有低成本的运行时递归接口，而不是“RustFlow 对所有 DAG 都比 Taskflow 慢 50 倍”。

### 3. Pipeline 路径的实现偏重

`data_pipeline` 和 `graph_pipeline` 主要暴露的是 pipeline 路径的额外开销。

RustFlow 这边有两个主要问题：

- `Pipeline::run` 和 `DataPipeline::run` 会先通过 `spawn_run_handle` 额外起一个协调线程
- `DataPipeline` 在 stage 间传递 payload 时使用 `Box<dyn Any + Send>`，会引入装箱、类型擦除、downcast 和 repeated payload reconstruction

而 Taskflow 的 pipeline benchmark 更接近静态类型的 pipeline 链。

这个差异在 `data_pipeline` 里尤其明显，因为它的 payload 会在八个 serial stage 之间反复变换类型。RustFlow 这边相当于每经过一站都要做：

- box
- downcast
- 重新 box

对于本身 stage work 并不算重的 benchmark 来说，这部分框架成本会非常显眼。

`graph_pipeline` 也是同样的情况，只不过它从另一个角度说明了这个问题：当 compute kernel 本身不够重时，pipeline framework 的开销就更容易被直接放大到最终结果里。

### 4. 少数 benchmark 当前并不是完全 apples-to-apples

这份报告依然有价值，但有几项 benchmark 不能直接被理解为“纯 runtime 对比”。

最重要的两个例子是：

- `for_each`
- `black_scholes`

在这两项里，RustFlow 并不是仅仅“做同样的事但调度器更慢”。更准确地说，是 Rust 侧 benchmark adapter 改变了 workload 结构，从而把额外的分配、收集和聚合成本也一起测进去了。

因此，这两个 benchmark 的慢值里，既有 runtime 因素，也有 benchmark 适配差异带来的放大效应。

## 分用例分析

### `async_task`

这是最纯粹的 runtime 固定开销 benchmark。

Taskflow 的 benchmark 做法是：

- 循环调用 `executor.silent_async(...)`
- 最后用 `executor.wait_for_all()` 等待全部完成

RustFlow 的 benchmark 表面上也是调用 `executor.silent_async(...)`，但 RustFlow 当前的 `silent_async` 内部并不是直接入线程池，而是临时构造一个单节点 `Flow`，再通过普通 flow 执行路径去跑。

根因：

- RustFlow 目前没有 one-off async task 的 direct fast path

判断：

- 这是 runtime 本身的真实瓶颈
- 不属于 benchmark 不公平问题

### `binary_tree`

这个 benchmark 里的 task 几乎没有实质工作，主要就是计数器加一。因此它比的几乎就是“建图 + 依赖处理 + 调度”的纯成本。

RustFlow 慢的原因主要有：

- 图在 Rust 侧动态构建
- 每轮执行前还要生成 graph snapshot
- 小 task 仍然要走完整的 flow 执行流程

判断：

- 这是典型的 runtime 固定成本问题
- 对大量细粒度 DAG 用户也有现实意义

### `black_scholes`

这个 benchmark 之所以慢，至少有两个因素叠加。

第一，RustFlow 这边走的是 `parallel_transform`，而当前 `parallel_transform` 会：

- 新建一个 `Flow`
- 切 chunk
- 分配输出容器
- 用 `Mutex<Option<_>>` 逐元素存储输出
- 最后再把所有结果收集回 `Vec`

第二，Taskflow 原 benchmark 的结构更有利。它会在一个 taskflow graph 内部循环运行，并直接把结果写入预分配好的数组。RustFlow 这边则是在外层重复调用 `parallel_transform`，之后再做 checksum 聚合。

这意味着 RustFlow 不只是慢在 scheduler 上，还额外承担了 adapter 层的分配和收集成本。

判断：

- 一部分是 `parallel_transform` 路径本身太重
- 一部分是 benchmark 适配方式不完全等价

### `data_pipeline`

这个 benchmark 主要暴露的是 pipeline 实现本身的成本，而不是 core executor 的问题。

RustFlow 当前 `DataPipeline` 的主要负担包括：

- `Box<dyn Any + Send>` 的动态 payload 传递
- 每个 stage 边界的 downcast
- run 级别额外协调线程

Taskflow 的 pipeline 更接近静态类型链，因此路径明显更短。

判断：

- 这是 RustFlow pipeline 设计上的真实性能问题
- 不能简单外推为“RustFlow executor 整体都慢”

### `embarrassing_parallelism`

这个 benchmark 的主导因素仍然是 tiny-task 调度开销。

Taskflow 那边基本只执行 `dummy(i)`。RustFlow 这边虽然 compute kernel 本身相近，但每个 task 还会额外执行一次原子加法来累加 checksum。这个原子操作不是主要瓶颈，但它确实属于额外工作，而且发生在计时区间内。

判断：

- 主要是细粒度调度问题
- 次要地被 benchmark 侧的 checksum 逻辑略微放大

### `fibonacci`

这是执行模型差异最明显的 benchmark 之一。

Taskflow 的做法是：

- 用 `task_group` 做运行时递归
- 只把一侧分支异步提交
- 另一侧在当前 worker 内继续计算
- 通过 `corun` 让 worker 在等待时保持工作

RustFlow 的做法是：

- 先递归构建完整 DAG
- 给每个结果节点分配一个 slot
- 父节点再从子节点 slot 中取值并合并

判断：

- 这是运行时递归支持缺失导致的真实能力差距
- 不能据此推出“所有 RustFlow 图都这么慢”

### `for_each`

这是当前报告里最不公平的 benchmark 之一。

Taskflow benchmark 的工作是：

- 对全局 vector 做原地 `tan` 变换

RustFlow benchmark 的工作则是：

- 创建输入 vector
- 填充随机值
- 调用 `parallel_transform`
- 分配新的输出 vector
- 对输出做 checksum

这两边实际上已经不是同一个 workload。

判断：

- 不能把它直接当作纯 scheduler 对比
- 当前大幅落后很大程度上是 benchmark adapter 结构差异造成的

### `graph_pipeline`

这个 benchmark 展示的是多阶段 pipeline 下的框架成本。

比较关键的一点是：RustFlow 的 compute kernel 并不明显比 C++ 参考实现更重。但在这种情况下，RustFlow 依然稳定慢约 1.8x 到 2.0x，说明瓶颈并不在 kernel，而在外围框架。

主要原因：

- pipeline 协调成本
- line 级任务启动成本
- atomic buffer 和 value handoff

判断：

- 这是 pipeline 实现层的真实问题

### `graph_traversal`

这也是典型的小任务 DAG benchmark。

由于 task body 被刻意设计得比较轻，因此 benchmark 会放大以下成本：

- 图构建成本
- 图快照成本
- task 调度成本
- visited 状态管理成本

RustFlow 还使用了原子 visited 数组，而 Taskflow 的 reference graph 是把 visited 状态直接存在 node 对象内部。

判断：

- 这是细粒度 DAG 下的真实 runtime 开销问题

### `integrate`

`integrate` 和 `fibonacci` 属于同一类结构问题，但通常更重，因为递归树规模会随着区间和误差条件迅速膨胀。

Taskflow 使用运行时递归加 `corun`，避免先建完整递归树。

RustFlow 这边则是：

- 先建完整递归 flow
- 为大量节点分配结果 slot
- 在真正算完之前先承担很重的建图和同步成本

判断：

- 这是运行时递归执行能力的真实差距

### `linear_chain`

这是最容易暴露固定调度成本的 DAG 之一。

原因很简单：linear chain 几乎没有可利用的并行度，任何时刻通常只有一个 runnable task。这样一来，调度开销就无法被宽前沿并行摊薄。

Taskflow 还使用了 `linearize` helper，而 RustFlow 这边仍然通过普通依赖推进链条逐个推进执行。

判断：

- 这是 runtime 固定成本偏高的典型表现

### `linear_pipeline`

这个 benchmark 需要和前面两个 pipeline 慢项区分来看。

除了最小的 size 点外，RustFlow 和 Taskflow 基本接近持平。这说明在 steady-state 下，这条 pipeline 路径并不是灾难性落后。

最小点偏慢，主要更像是启动成本问题，例如：

- 协调线程创建
- line worker 启动
- stop 传播成本

判断：

- 属于小问题
- 目前不是最高优先级

## 分类归纳

### A. 真实的 runtime 固定开销问题

- `async_task`
- `binary_tree`
- `linear_chain`
- `graph_traversal`

### B. 运行时递归能力缺口

- `fibonacci`
- `integrate`

### C. Pipeline 实现偏重

- `data_pipeline`
- `graph_pipeline`
- `linear_pipeline` 的小规模点

### D. 当前 benchmark 适配放大了差距

- `for_each`
- `black_scholes`
- 次一级是 `embarrassing_parallelism`

## 优先级建议

### 优先级 1：先修 benchmark 公平性

在真正优化 runtime 之前，建议先把最明显不公平的 benchmark adapter 改成更接近原始 Taskflow benchmark 的结构。

建议改动：

- `for_each` 改为原地 `parallel_for`，不要再走 `parallel_transform + checksum`
- `black_scholes` 改为写入预分配输出 buffer，并尽量匹配 Taskflow 的 `NUM_RUNS` 内部循环结构
- 纯校验性质的 checksum 尽量移出计时区，或保证两边做完全同等的校验工作

收益：

- 先剥离“假的差距”
- 后续优化前后的报告会更可信

### 优先级 2：给 async 补 direct fast path

为 `async_task` 和 `silent_async` 增加直接入队 executor 的路径，不再为每个 one-off async 临时创建 `Flow`。

收益：

- `async_task` 会直接改善
- `linear_chain`、`binary_tree`、`embarrassing_parallelism`、`graph_traversal` 也会明显受益

### 优先级 3：补运行时递归接口

增加一个类似 Taskflow `task_group + corun` 的轻量运行时递归机制。

收益：

- `fibonacci` 会有决定性改善
- `integrate` 会有决定性改善

### 优先级 4：瘦身 `parallel_transform`

当前实现的主要问题是：

- 每个输出元素一个 `Mutex<Option<U>>`
- 输出完成后还要重新收集成 `Vec`
- auto chunk 策略依据机器总并发度，而不是当前 executor worker 数

其中最后一点尤其值得注意：当前 `ParallelForOptions::Auto` 基于 `available_parallelism` 分 chunk，而 benchmark 报告用的是固定 4 个 worker。如果宿主机逻辑核更多，就会切出比实际 executor 更碎的 chunk。

收益：

- `for_each` 会改善
- `black_scholes` 会改善
- 其他 transform 类算法也会受益

### 优先级 5：继续优化 pipeline

比较大的结构性收益点包括：

- 去掉 `Pipeline::run` / `DataPipeline::run` 的额外协调线程
- 避免 `Box<dyn Any + Send>` 的动态 payload 路径
- 降低 serial stage 的同步和 bookkeeping 成本

收益：

- `data_pipeline` 会明显改善
- `graph_pipeline` 会明显改善
- `linear_pipeline` 的启动成本也会下降

## 最终判断

当前报告并不支持“RustFlow 在 CPU 并行计算上普遍慢于 Taskflow”这个结论。它更准确地说明了三件事：

- RustFlow 在超细粒度任务上的固定成本偏高
- RustFlow 还缺少适合分治递归的运行时执行接口
- RustFlow 的 pipeline 路径还不够轻

同时，报告里也有两类 benchmark 当前确实把 Rust 侧 adapter 的额外工作测进去了：

- `for_each`
- `black_scholes`

因此，最值得的下一步顺序是：

1. 先修 benchmark 等价性
2. 再给 executor async 增加 direct fast path
3. 最后再根据新的报告决定递归和 pipeline 优化的优先级细节

这个顺序既能先把报告里的“伪差距”剥掉，也能最快压低当前最显眼的红项。
