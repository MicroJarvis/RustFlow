# RustFlow Epics

## E1. Core Graph And Executor

目标：交付可重复运行的基础 `Flow` 图模型和可用的并发执行器。

范围：

- `Flow` / `FlowBuilder` / `TaskHandle`
- placeholder 和 static task
- 依赖编排
- `Executor::run` / `run_n` / `run_until` / `wait_for_all`
- `FlowError`
- DOT 导出
- 基础 work-stealing 调度

验收标准：

- 线性 DAG、fork-join、空图、重复运行都能正确执行
- 任务 panic 可以通过 run handle 返回
- 多 worker 下每个任务只执行一次

当前状态：`Done`

## E2. Control Flow And Dynamic Scheduling

目标：让 `Flow` 具备 Taskflow 风格的控制流表达能力，而不只是静态 DAG。

范围：

- `spawn_condition`
- `spawn_multi_condition`
- 条件反馈循环
- `subflow`
- 动态子图 join 语义
- detached subflow 在第一阶段保持关闭

验收标准：

- 单条件、多条件、循环反馈都能正确结束
- `subflow` 子图可嵌套，且默认 join 语义正确
- 控制流不会导致 run handle 提前完成或永久挂起

当前状态：`Done`

## E3. Runtime, Async, And Composition

目标：补齐运行时动态调度能力，使 RustFlow 接近 C++ Taskflow 的表达上限。

范围：

- `RuntimeCtx`
- runtime 内 `schedule`
- runtime 内 `async_task` / `silent_async`
- `corun`
- `dependent_async`
- `compose`

验收标准：

- runtime 中可以调度活跃任务
- corun 不阻塞 worker，不引入死锁
- dependent async 的依赖推进正确
- 组合图可以重复运行且不污染模板图

当前状态：`Done`

## E4. Reliability, Limits, And Observability

目标：补齐取消、限流、观察和错误边界，保证内核可以安全用于其他项目。

范围：

- cooperative cancellation
- semaphore / limited concurrency
- observer hooks
- worker 生命周期观测
- 更完整的错误模型和状态检查

验收标准：

- cancel 后未激活后继不再推进
- semaphore 可以限制并发数
- observer 能看到 task start/finish 和 worker sleep/wake
- 异常和取消不会让 topology 卡死

当前状态：`Done`

## E5. Algorithms And Performance

目标：在统一内核上补齐常用并行算法，并建立性能对比基线。

范围：

- `parallel_for`
- `for_each`
- `reduce`
- `transform`
- `find`
- `scan`
- `sort`
- benchmark harness

验收标准：

- 算法结果与串行实现一致
- 算法层建立在 `flow-core` 之上，而不是旁路实现
- 对普通 threadpool 明显更优
- 常见数据并行场景接近 `rayon`

当前状态：`Done`

## E6. Phase 2 Pipeline And Profiling

目标：在核心稳定后补齐流水线、性能分析和更强的生态接口。

范围：

- `Pipeline` / `ScalablePipeline` / `DataPipeline`
- trace / profiling
- graph validation / introspection
- priority / partitioner 增强
- `compat` facade
- 异构执行实验扩展点

验收标准：

- pipeline token 调度正确
- 可导出 trace 用于性能回归分析
- 大图可以做基本诊断
- 兼容层不污染主 API

当前状态：`Done`
