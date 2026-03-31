# RustFlow Stories

状态说明：

- `Done`
- `In Progress`
- `Planned`

## E1. Core Graph And Executor

### S1. Create Workspace And Core Crate

状态：`Done`

内容：

- 建立 workspace
- 建立 `flow-core`
- 建立 `flow-algorithms`
- 建立开发计划文档

验收标准：

- `cargo test --workspace` 可运行
- crate 命名和目录命名符合当前规范

### S2. Implement Basic Flow Graph API

状态：`Done`

内容：

- `Flow`
- `FlowBuilder`
- `TaskHandle`
- `TaskId`
- placeholder
- 任务命名

验收标准：

- 可以构建线性和 fork-join 图
- 可生成 DOT 文本

### S3. Implement Initial Executor

状态：`Done`

内容：

- worker 池
- 本地队列
- stealing
- `run`
- `run_n`
- `run_until`
- `wait_for_all`

验收标准：

- 多 worker 下任务全部完成
- 不重复执行任务

### S4. Handle Panic As FlowError

状态：`Done`

内容：

- 捕获任务 panic
- 返回 `FlowError`

验收标准：

- run handle 可见错误
- 错误中包含任务上下文

## E2. Control Flow And Dynamic Scheduling

### S5. Implement Condition Task

状态：`Done`

内容：

- `spawn_condition`
- 按分支位置选择单个后继

验收标准：

- 单条件只激活一个后继

### S6. Implement Multi-Condition Task

状态：`Done`

内容：

- `spawn_multi_condition`
- 一次激活多个后继

验收标准：

- 多条件可以激活多个目标分支

### S7. Support Condition Feedback Loops

状态：`Done`

内容：

- 调整运行完成判定
- 支持条件自反馈循环

验收标准：

- 循环可以重复进入并正确退出
- run handle 不会提前完成

### S8. Implement Subflow Join Semantics

状态：`Done`

内容：

- `spawn_subflow`
- 子图 builder
- 默认 join 语义

验收标准：

- 父任务只有在子图完成后才算完成
- 子图支持嵌套

### S9. Implement Detached Subflow Policy

状态：`Done`

内容：

- 第一阶段明确不开放 detached subflow
- 保留默认 join 语义作为唯一公开行为
- detached 相关生命周期和完成语义延后到后续阶段再讨论

验收标准：

- 行为定义清晰
- 不破坏默认 join 模型

## E3. Runtime, Async, And Composition

### S10. Implement Runtime Context

状态：`Done`

内容：

- `RuntimeCtx`
- 在任务执行中拿到调度上下文

验收标准：

- runtime task 可以访问 executor 和当前运行上下文

### S11. Implement Runtime Schedule

状态：`Done`

内容：

- 运行中手动调度活跃任务

验收标准：

- 被调度任务可以立即进入可执行队列

### S12. Implement Corun

状态：`Done`

内容：

- worker 参与目标图执行直到目标完成

验收标准：

- 不阻塞 worker
- 不引入死锁

### S13. Implement Executor Async

状态：`Done`

内容：

- executor 级 `async_task`
- `silent_async`

验收标准：

- 支持有返回值和无返回值两种路径

### S14. Implement Dependent Async

状态：`Done`

内容：

- 依赖计数驱动的 async 扇入

验收标准：

- 依赖完成后才触发执行

### S15. Implement Flow Composition

状态：`Done`

内容：

- `compose`
- 组合子图模板

验收标准：

- 组合图可以重复运行
- 不污染静态模板图

## E4. Reliability, Limits, And Observability

### S16. Implement Cooperative Cancellation

状态：`Done`

内容：

- run handle cancel
- runtime cancel check

验收标准：

- 未激活后继停止推进

### S17. Implement Semaphores

状态：`Done`

内容：

- 任务级并发限流

验收标准：

- 并发数不超过配置上限

### S18. Implement Observer Hooks

状态：`Done`

内容：

- task start / finish
- worker sleep / wake

验收标准：

- observer 可看到完整生命周期事件

## E5. Algorithms And Performance

### S19. Implement Parallel For

状态：`Done`

验收标准：

- 结果正确
- 支持基本分块策略

### S20. Implement Reduce And Transform

状态：`Done`

验收标准：

- 数值结果正确
- 支持多 worker 并行执行

### S21. Implement Find, Scan, And Sort

状态：`Done`

验收标准：

- 与串行结果一致

### S22. Build Benchmark Harness

状态：`Done`

内容：

- 对比普通 threadpool
- 对比 `rayon`
- 对比 C++ Taskflow CPU 子集

验收标准：

- benchmark 可重复运行
- 输出可用于性能回归

## E6. Phase 2 Pipeline And Profiling

### S23. Implement Pipeline MVP

状态：`Done`

内容：

- serial / parallel pipe
- stop token

验收标准：

- 线性 pipeline 可以工作

### S24. Implement Scalable And Data Pipeline

状态：`Done`

验收标准：

- token 和 defer 语义正确

### S25. Implement Trace Export

状态：`Done`

验收标准：

- 可导出 timeline 数据

### S26. Implement Graph Validation And Introspection

状态：`Done`

验收标准：

- 能做基本图诊断

### S27. Implement Compat Facade

状态：`Done`

验收标准：

- 提供接近 C++ Taskflow 的 facade
- 不污染主 API
