# RustFlow Development Plan

这个目录下的开发计划已经拆分为两个层级：

- [Epics](./epics.md)
  - 面向阶段目标、能力边界和里程碑
- [Stories](./stories.md)
  - 面向可执行任务、验收标准和当前状态

## Planning Principles

- 以 C++ Taskflow 的 CPU 核心能力为参考目标
- 对外 API 使用 Rust 原生风格，核心对象命名为 `Flow`
- 严格采用 TDD：先测试，再实现，再重构
- 先交付可复用的核心调度器，再叠加动态图、算法层和观测能力
- 第二期能力不能破坏第一期核心调度器的稳定性

## Current Structure

- `flow-core`
  - 核心图模型、执行器、runtime、取消、观测、pipeline、compat、测试
- `flow-algorithms`
  - 基于 `flow-core` 的并行算法层，已覆盖 `parallel_for`、`reduce`、`transform`、`find`、`scan`、`sort`
- `benchmarks`
  - 对比 `flow` / `threadpool` / `rayon` / C++ `taskflow` 的 benchmark harness
- `taskflow/`
  - 本地 C++ 参考仓库，只做架构和行为对照

## Current Progress

- `doc/epics.md` 中 E1 到 E6 的能力已经完成并落到代码
- workspace 现已覆盖静态 DAG、控制流、subflow、runtime、async、compose、取消、限流、观测、pipeline、算法层和 benchmark harness
- `cargo test --workspace` 当前可通过
- 当前更需要收口文档、CI、性能基线和对外发布准备，而不是继续补核心执行语义

## Documents

- [Epics](./epics.md)
- [Stories](./stories.md)
- [Latest Taskflow Performance Plan](./plans/2026-04-05-post-lockfree-queue-performance-plan.md)
- [Latest Post-Lockfree Progress Report](./plans/2026-04-05-post-lockfree-progress-report.md)
