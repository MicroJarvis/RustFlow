# RustFlow 第二轮性能优化计划

## 背景：第一轮优化成果与剩余差距

第一轮（2026-04-06）实施了 4 项优化：放宽内存序、内联 work-stealing fast path、轻量 Task 表示、Pipeline token 批处理。

### 当前 Benchmark 状态 (4 threads, 3 rounds)

| 分类 | Benchmark | 第一轮前 | 第一轮后 | 差距 |
|------|-----------|---------|---------|------|
| **递归/task 密集** | fibonacci @20 | 12.4x | **5.6x** | 仍有 5.6x 差距 |
| | binary_tree @16384 | 3.5x | **1.85x** | 接近目标 |
| | thread_pool @100 | 5.7x | **3.6x** | 仍有 3.6x 差距 |
| | linear_chain @65536 | 3.5x | 3.3x | 未改善 |
| **Pipeline** | data_pipeline @1024 | 5.6x | 3.6x | 仍有 3.6x 差距 |
| | linear_pipeline @1024 | 3.8x | 3.7x | 未改善 |
| | graph_pipeline @12904 | 3.8x | 5.3x | 反而恶化 |
| **算法** | scan @100K | 6.3x | **1.6x** | 大幅改善 |
| | sort @100K | 3.1x | **2.5x** | 略有改善 |
| | mandelbrot @300 | 3.3x | **1.3x** | 大幅改善 |
| **并行** | embarrassing_par @32K | 1.8x | **1.0x** | 已达对等 |
| | wavefront @1024 | 4.0x | **4.0x** | 未改善 |

### 剩余瓶颈分析

| 瓶颈根因 | 影响的 Case | 严重程度 |
|----------|------------|---------|
| Pipeline serial stage 的 per-token SpinLock | data/linear/graph pipeline | 高 — 3.6-5.3x |
| fibonacci 的递归 task spawn 路径仍有开销 | fibonacci, thread_pool | 高 — 3.6-5.6x |
| wavefront 的细粒度 DAG 任务 + 依赖解析 | wavefront | 高 — 4.0x |
| `RuntimeJoinScope` 的 Arc 分配 | 所有 runtime 任务 | 中 |
| 内联 graph 执行的 `pending` 向量重建 | subflow, fibonacci | 中 |
| corun_children 的 busy-wait 轮询 | pipeline, 并行算法 | 中 |

---

## 第二轮优化方案

### 实施顺序

```
Phase 1 (低风险快速收益):
  ├── 优化A: Pipeline SpinLock 自适应退避
  └── 优化B: RuntimeJoinScope 延迟分配

Phase 2 (中等复杂度):
  ├── 优化C: Pipeline line runner 协作式调度
  └── 优化D: 内联 graph 执行路径优化

Phase 3 (架构级):
  └── 优化E: corun_children 事件通知替代轮询
```

### 本轮边界与执行约束

本轮优化默认维持现有 `Pipeline` / `DataPipeline` 语义，不做 Taskflow 风格的结构性重写（例如统一 per-line typed buffer、`join_counter` 驱动的整套调度替换）。原因是这类方案虽然潜在收益更大，但会同时改变数据布局和并发推进模型，不适合作为第二轮的主线。

本轮实现必须保持以下外部可见约束：

- `PipeContext.token()` 继续保持全局单调递增语义，而不是“每条 line 自己递增”
- `stop()` 仍然只允许在 stage 0 / pipe 0 调用，且 stop 后不再继续发放新 token
- `num_tokens()` 仍然表示本次 run 实际成功发放的 token 数量
- 运行中的 `RunHandle.cancel()` 必须继续生效，不能退化成“仅在 run 开始时读取一次 cancelled 状态”

对上述约束有破坏风险的方案，只能作为候选分支验证，不能默认进入主线实施顺序。

---

## 优化A: Pipeline SpinLock 自适应退避

### 问题

`acquire_serial_stage` (pipeline.rs) 对 serial stage 的锁获取使用硬编码 64 次自旋 → yield 策略。对于 8-line pipeline 中的 8 个 serial stage，每个 token 需要 8 次锁获取。在高竞争场景下（多 line 同时争抢同一 stage 锁），64 次自旋浪费大量 CPU 周期。

当前热路径 per-token 原子操作分解：
- `should_stop_issuing_tokens()`: 3 次 atomic load（stop + abort + cancelled）
- stage 0 lock CAS: 1 次（成功时）或 N 次（竞争时）+ 最多 64 次 spin_loop
- `next_token` load + store: 2 次
- stages 1-7 各 1 次 CAS
- 总计：最低 ~13 次原子操作/token，竞争时指数增长

### 方案

**A1: 降低多 line 场景的自旋次数**

```rust
fn acquire_serial_stage(
    lock: &SpinLock,
    run_state: &PipelineRunState,
    is_first_stage: bool,
) -> SerialStageAcquire {
    // 快速路径：第一次尝试就成功
    if let Some(guard) = lock.try_lock() {
        return SerialStageAcquire::Acquired(guard);
    }

    // 自适应退避：对非首 stage 减少自旋（锁持有时间短）
    let max_spins = if is_first_stage { 32 } else { 8 };
    let mut spins = 0;

    loop {
        if run_state.should_abort_immediately() {
            return SerialStageAcquire::Aborted;
        }
        if let Some(guard) = lock.try_lock() {
            return SerialStageAcquire::Acquired(guard);
        }
        spins += 1;
        if spins < max_spins {
            std::hint::spin_loop();
        } else {
            spins = 0;
            std::thread::yield_now();
        }
    }
}
```

**A2: 合并本地 stop/abort 状态，但保留 live cancelled**

`stop_requested` 和 `abort_requested` 是 pipeline 本地状态，可以合并为单个 `AtomicU8`。但 `cancelled` 不能在 run 开始时复制进本地 flags，否则运行中发出的 cancel 可能丢失或明显延迟。

```rust
struct PipelineRunState {
    next_token: AtomicUsize,
    /// Bits: 0=stop, 1=abort
    flags: AtomicU8,
    cancelled: Arc<AtomicBool>,
    stage_locks: Vec<Option<SpinLock>>,
}

impl PipelineRunState {
    fn should_stop_issuing_tokens(&self) -> bool {
        self.flags.load(Ordering::Acquire) != 0
            || self.cancelled.load(Ordering::Acquire)
    }

    fn request_stop(&self) {
        self.flags.fetch_or(0b001, Ordering::Release);
    }

    fn request_abort(&self) {
        self.flags.fetch_or(0b010, Ordering::Release);
    }
}
```

这样可以把本地 `stop + abort` 的两次 load 合并为一次，同时保留运行中 cancel 的实时可见性。保守估计，`should_stop_issuing_tokens()` 从 3 次 atomic load 降为 2 次，而不是激进地降为 1 次。

**A3: 减少 should_abort_immediately 检查频率**

在 `acquire_serial_stage` 自旋循环中，不需要每次自旋都检查 abort。改为每 N 次自旋检查一次：

```rust
if spins % 8 == 0 && run_state.should_abort_immediately() {
    return SerialStageAcquire::Aborted;
}
```

### 修改文件

- `flow-core/src/pipeline.rs`
  - `PipelineRunState` 结构体 — 合并 stop/abort flags，保留 live cancelled
  - `acquire_serial_stage` — 自适应退避
  - `should_stop_issuing_tokens` / `should_abort_immediately` — 位掩码实现

### 预期收益

| Case | 当前 | 预期 | 原因 |
|------|------|------|------|
| data_pipeline @1024 | 3.6x | 2.8-3.3x | 减少自旋浪费 + 合并本地 atomic load |
| linear_pipeline @1024 | 3.7x | 3.1-3.5x | 同上 |

### 风险

- 低到中：仅修改内部实现，但退避参数对不同平台敏感，过早 `yield_now()` 可能让低竞争场景变慢
- `A2` 只能合并本地 stop/abort；若把 `cancelled` 也改成 run 开始时复制，会破坏运行中 cancel 语义
- `A3` 会增加 abort/cancel 的响应延迟，需要确认不会导致额外执行过多 stage

---

## 优化B: RuntimeJoinScope 延迟分配

### 问题

每个 runtime task 和 async task 执行时都分配 `Arc<RuntimeJoinScope>`（executor.rs L1177, L1470），即使绝大部分 task 从不调用 `silent_async` 或 `corun_children`。

`RuntimeJoinScope` 本身很小（2 个 atomic 字段），但 Arc 的堆分配 + 引用计数仍有开销。对于 fibonacci(20) 的 ~21K 个 task，这是 ~21K 次不必要的 Arc 分配。

### 方案

将 `RuntimeJoinScope` 从 Arc 预分配改为按需创建。由于 `RuntimeCtx` 在运行期通常通过共享引用传递，不能依赖 `&mut self` 的惰性初始化接口，建议直接使用 `OnceLock<Arc<RuntimeJoinScope>>`：

```rust
pub struct RuntimeCtx {
    executor: Executor,
    worker_id: usize,
    scheduler: Option<Arc<dyn Fn(TaskId) + Send + Sync>>,
    cancelled: Arc<AtomicBool>,
    // 改为 OnceLock，按需创建
    join_scope: OnceLock<Arc<RuntimeJoinScope>>,
}

impl RuntimeCtx {
    /// 获取或创建 join_scope（惰性初始化）
    fn ensure_join_scope(&self) -> &Arc<RuntimeJoinScope> {
        self.join_scope.get_or_init(|| Arc::new(RuntimeJoinScope::default()))
    }

    pub fn silent_async<F>(&self, task: F) where ... {
        // 只在实际 spawn 子任务时才创建 join_scope
        let scope = self.ensure_join_scope();
        scope.add_child();
        // ...
    }

    pub fn corun_children(&self) -> Result<(), FlowError> {
        match self.join_scope.get() {
            Some(scope) if !scope.is_idle() => {
                self.executor.wait_until_inline(self.worker_id, || scope.is_idle());
                scope.take_error().map_or(Ok(()), Err)
            }
            _ => Ok(()), // 没有子任务，直接返回
        }
    }
}
```

### 修改文件

- `flow-core/src/runtime.rs` — `RuntimeCtx` 结构体改为 Option
- `flow-core/src/executor.rs` — `execute_node_inline` 和 `execute_async` 中移除预分配

### 预期收益

| Case | 预期改善 |
|------|---------|
| fibonacci @20 | 5-10%（减少 ~21K 次 Arc 分配）|
| thread_pool @100 | 3-5% |
| 所有 runtime 任务路径 | 2-5% |

### 风险

- 低：语义不变，只是延迟初始化
- 需要同时修改 `execute_node_inline` 和 `execute_async` 的 runtime 构造路径，避免保留无意义的预分配
- `corun_children()`、`silent_async()`、`silent_result_async()` 都要走同一套 `ensure_join_scope()`，避免出现一部分路径仍然强制分配

---

## 优化C: Pipeline Line Runner 协作式调度

### 问题

当前 pipeline 的每条 line（除 line 0 外）都通过 `runtime.silent_result_async` 生成一个独立的异步任务。这导致：

1. N-1 次 `Box<dyn FnOnce>` 分配（闭包堆分配）
2. N-1 次 `Arc::clone(&run_state)` 和 `Arc::clone(&pipes)`
3. N-1 次 `schedule_runtime_silent_child` 调用（进入 executor 调度路径）
4. 每条 line 的整个 `run_pipe_line` 执行完后才回到 corun_children 等待

对于 data_pipeline（8 lines × 1024 tokens），line 的启动开销虽然是一次性的，但 line 之间的协调（stage lock 竞争）是瓶颈。

### 方案

**C1: 缩小 line runner 启动开销**

当前 executor 的 async 接口要求 `Box<dyn FnOnce(...)> + 'static`，因此“完全预分配闭包并零分配复用”并不现实。更现实的目标是：

- 让每条 line 的闭包捕获尽量小，只携带 `line` 和共享状态句柄
- 复用共享的 stages / pipes 存储，避免在 run 热路径中重建额外容器
- 若没有观测到明显收益，则 C1 可以直接跳过，不强行实现

```rust
runtime.silent_result_async(move |_| {
    run_data_line(Arc::clone(&stages), line, Arc::clone(&run_state))
});

// 保持 API 不变，但保证闭包环境最小化
```

**C2: 减少 per-line Arc 克隆（延期）**

当前每条 line 克隆 `Arc<[Pipe]>` 和 `Arc<PipelineRunState>`。可以通过将 line runner 改为接受引用来消除：

```rust
// 使用 scoped threads 或 crossbeam::scope 来避免 Arc
// 但需要 executor 支持 scoped task spawning
```

这需要 executor 支持非 `'static` 的 scoped spawn，已经超出第二轮范围。本轮不做。

**C3: Token 分发去中心化（候选分支，不纳入主线）**

当前所有 line 通过同一个 `next_token` atomic 竞争 token。可以改为 round-robin 预分配：

```rust
// 预分配 token 给每条 line
// line 0 gets tokens 0, N, 2N, ...
// line 1 gets tokens 1, N+1, 2N+1, ...
// line K gets tokens K, N+K, 2N+K, ...
// 其中 N = num_lines

fn run_pipe_line_with_stride(
    pipes: Arc<[Pipe]>,
    line: usize,
    num_lines: usize,
    run_state: Arc<PipelineRunState>,
) -> Result<(), FlowError> {
    let mut token = line; // 从自己的 line ID 开始
    loop {
        if run_state.should_stop_issuing_tokens() {
            return Ok(());
        }
        // 不再竞争 next_token，直接用自己的 stride
        let mut context = PipeContext { line, pipe: 0, token, stop_requested: false };
        // ... stage 0 仍需锁来序列化（保证 token ordering）
        // 但 next_token 的 load/store 可以消除
        token += num_lines;
    }
}
```

这类 stride 分发最大的风险不是统计，而是语义：

- 当前实现中，token 是在 stage 0 拿到串行权限后统一发放，`PipeContext.token()` 对外表现为全局单调递增
- stride 方案会把 token 归属固定到某条 line，哪条 line 先拿到 stage 0，就可能先发更大的 token，破坏现有顺序语义
- `stop()` 的边界也会变复杂：需要明确“停止发 token”与“各 line 已经预留但尚未执行的 token”之间的关系

因此 C3 只能作为单独实验分支验证；若没有附加调度机制来重建全局 token 次序，不进入主线。

### 修改文件

- `flow-core/src/pipeline.rs`
  - `run_pipe_chain` / `run_data_chain` — 仅做 line runner 启动路径瘦身
  - `run_pipe_line` / `run_data_line` — 不改变 token 发放语义
  - `C3` 如需实验，必须放到单独分支，不与主线 Phase 2 混做

### 预期收益

| Case | 当前 | 预期（仅 C1） | 候选（C3，需单独验证） |
|------|------|---------------|-------------------------|
| data_pipeline @1024 | 3.6x | 3.3-3.5x | 仅在不破坏 token 语义时评估 |
| linear_pipeline @1024 | 3.7x | 3.4-3.6x | 同上 |

### 风险

- C1：低风险，但可能收益很小，需要先用 micro-benchmark 证明 line 启动分配确实可见
- C2：高复杂度，超出本轮范围
- C3：高风险 — 可能改变 `PipeContext.token()` 顺序、`stop()` 边界与 `num_tokens()` 统计语义

---

## 优化D: 内联 Graph 执行路径优化

### 问题

`execute_graph_inline`（executor.rs L1530-1600）用于 subflow 和内联 graph 执行。每次调用都：

1. 重建 `pending: Vec<usize>` 向量（L1542-1546）— 堆分配 + 遍历所有节点
2. 分配 `VecDeque<usize>` 作为任务队列（L1547）— 堆分配
3. 对每个 runtime task 分配 `Arc<RuntimeJoinScope>`（L1470）

对 fibonacci(20) 来说，递归 subflow 模式下，这些路径被频繁调用。

### 方案

**D1: 小图优化 — 栈上执行**

对于节点数 <= 32 的小图（涵盖大部分 subflow），使用栈上固定大小数组：

```rust
fn execute_graph_inline(/* ... */) {
    let node_count = graph.nodes.len();

    if node_count <= 32 {
        // 栈上路径：无堆分配
        let mut pending = [0usize; 32];
        for (i, node) in graph.nodes.iter().enumerate() {
            pending[i] = node.predecessor_count;
        }
        let mut queue = arrayvec::ArrayVec::<usize, 32>::new();
        for &root in &graph.roots {
            queue.push(root);
        }
        // ... 执行逻辑与堆分配版本相同
    } else {
        // 大图：现有堆分配路径
        let mut pending: Vec<usize> = graph.nodes.iter()
            .map(|node| node.predecessor_count).collect();
        // ...
    }
}
```

**D2: 缓存 pending 初始值**

在 `GraphSnapshot` 中预计算并缓存 pending 初始值：

```rust
struct GraphSnapshot {
    nodes: Vec<NodeSnapshot>,
    roots: Vec<usize>,
    /// 预计算的 predecessor_count 向量，避免每次 execute 时重新收集
    initial_pending: Vec<usize>,
}
```

这样 `execute_graph_inline` 只需 `clone()` 一份：

```rust
let mut pending = graph.initial_pending.clone(); // Vec clone 比 iter().map().collect() 快
```

**D3: Fibonacci 特化路径**

Fibonacci benchmark 的 TaskGroup + silent_async + corun 模式的瓶颈在于：
- 每次 `silent_async` 创建 `ScheduledAsyncTask`（含 `Box<dyn FnOnce>` + 多个 Arc clone）
- `corun_children` 进入 `wait_until_inline` 的 spin 循环

可以为 "spawn one child, execute other inline, join" 模式提供特化 API：

```rust
impl RuntimeCtx {
    /// 并行执行两个任务：f1 异步 spawn，f2 内联执行，然后等待两者完成
    pub fn fork_join<F1, F2, R1, R2>(&self, f1: F1, f2: F2) -> (R1, R2)
    where
        F1: FnOnce(&RuntimeCtx) -> R1 + Send + 'static,
        F2: FnOnce(&RuntimeCtx) -> R2,
        R1: Send + 'static,
    {
        // f1: 轻量 spawn（避免完整的 ScheduledAsyncTask 路径）
        // f2: 内联执行
        // join: 等待 f1 完成
    }
}
```

### 修改文件

- `flow-core/src/executor.rs` — `execute_graph_inline` 小图优化 + pending 缓存
- `flow-core/src/graph.rs`（或 `GraphSnapshot`）— 添加 `initial_pending`
- `flow-core/src/runtime.rs` — `fork_join` API（可选）

### 预期收益

| Case | 当前 | 预期（D1+D2） | 预期（+D3） |
|------|------|-------------|-----------|
| fibonacci @20 | 5.6x | 4.0-5.0x | 2.5-3.5x |
| wavefront @1024 | 4.0x | 3.0-3.5x | — |

### 风险

- D1: 低 — 纯内部优化，行为不变
- D2: 低 — GraphSnapshot 是不可变的，预计算安全
- D3: 中 — 新 API，需要设计好泛型约束和错误处理

---

## 优化E: corun_children 事件通知替代轮询

### 问题

当前 `corun_children` 调用 `wait_until_inline`，后者在 `DriveMode::NoPark` 模式下执行：
1. 尝试 `next_task` 偷取工作
2. 如果没有工作，自旋 64 次（`spin_loop()`）
3. 然后 `yield_now()`
4. 循环直到 `done()` 返回 true

这是 busy-wait 模式，在等待子任务完成的同时做 work-stealing。问题是：
- 如果没有可偷的工作，纯粹是空转浪费 CPU
- `yield_now()` 的代价在 Linux/macOS 上不同（macOS 上更重）
- 对于 pipeline 的 `corun_children`，常常等待 7 条 line 完成，期间 line 自己在处理 token，work-stealing 的收益有限

### 方案

**E1: 使用条件变量通知等待者，而不是裸 `Thread*` 指针**

不建议在 `RuntimeJoinScope` 中直接保存 `AtomicPtr<Thread>`。原因是 waiter 的生命周期和 `corun_children()` 栈帧绑定，裸指针极易形成悬垂引用；同时若在“检查 idle”与“登记 waiter”之间发生最后一个 child 完成，还会出现 lost wakeup。

更稳妥的方案是给 `RuntimeJoinScope` 增加受保护的等待原语，在最后一个 child 完成时 `notify_one()`：

```rust
pub(crate) struct RuntimeJoinScope {
    pending_children: AtomicUsize,
    has_error: AtomicBool,
    wait_lock: Mutex<()>,
    wait_cv: Condvar,
}

impl RuntimeJoinScope {
    pub(crate) fn finish_child(&self, result: Result<(), FlowError>) {
        if result.is_err() {
            self.has_error.store(true, Ordering::Release);
        }
        if self.pending_children.fetch_sub(1, Ordering::AcqRel) == 1 {
            let _guard = self.wait_lock.lock().unwrap();
            self.wait_cv.notify_one();
        }
    }
}
```

修改 `corun_children`：

```rust
pub fn corun_children(&self) -> Result<(), FlowError> {
    if self.join_scope.is_idle() {
        return self.join_scope.take_error().map_or(Ok(()), Err);
    }

    loop {
        self.executor.wait_until_inline_short(self.worker_id, || self.join_scope.is_idle());
        if self.join_scope.is_idle() {
            break;
        }

        let guard = self.join_scope.wait_lock.lock().unwrap();
        if !self.join_scope.is_idle() {
            let _ = self.join_scope.wait_cv.wait_timeout(guard, Duration::from_micros(10));
        }
    }

    self.join_scope.take_error().map_or(Ok(()), Err)
}
```

**E2: 混合策略 — 短暂 work-stealing + scope-assisted timed wait**

不完全去除 work-stealing，而是在连续若干次 steal 失败后，切换到由 `RuntimeJoinScope` 协助的短暂等待。这里不要直接对 worker 使用裸 `park()` / `unpark()` 协议，优先通过受保护的等待原语或 executor 自己的 parking 协议实现。

```rust
DriveMode::NoParkShort => {
    steal_attempts += 1;
    if steal_attempts < 8 {  // 只自旋 8 次（而非 64）
        std::hint::spin_loop();
    } else {
        // 交给 join_scope 的 timed wait，避免纯空转
        wait_on_join_scope_for(Duration::from_micros(10));
        steal_attempts = 0;
    }
}
```

### 修改文件

- `flow-core/src/runtime.rs` — `RuntimeJoinScope` 添加 wait primitive + `corun_children` 改造
- `flow-core/src/executor.rs` — `drive_worker_until` 添加 NoParkShort 模式

### 预期收益

| Case | 当前 | 预期 |
|------|------|------|
| pipeline 系列 | 3.6-5.3x | 2.5-3.5x（减少空转开销）|
| fibonacci @20 | 5.6x | 4.0-5.0x |
| 所有 corun 路径 | — | 降低 CPU 使用率，间接提速 |

### 风险

- 中高：涉及等待协议与 work-stealing 的平衡，需要仔细处理竞态条件
- 不能使用裸 waiter 指针，否则容易出现悬垂引用和 lost wakeup
- 如果等待策略过于激进，会削弱等待期间的 work-stealing，导致延迟升高或吞吐下降
- 需要特别验证 nested `corun_children`、child panic/error、cancel 与 pipeline stop 交错的情况

---

## 综合影响预估

| Benchmark | 当前 | Phase 1 后 | Phase 2 后 | Phase 3 后 |
|-----------|------|-----------|-----------|-----------|
| fibonacci @20 | 5.6x | 4.5-5.0x | 3.5-4.5x | 2.8-4.0x |
| data_pipeline @1024 | 3.6x | 2.8-3.3x | 2.7-3.2x | 2.2-3.0x |
| linear_pipeline @1024 | 3.7x | 3.1-3.5x | 3.0-3.4x | 2.5-3.2x |
| graph_pipeline @12904 | 5.3x | 4.0-4.5x | 3.0-3.5x | 2.5-3.2x |
| thread_pool @100 | 3.6x | 3.0-3.3x | 2.5-3.0x | 2.0-2.8x |
| wavefront @1024 | 4.0x | 3.5x | 3.0x | 2.5-3.0x |
| binary_tree @16384 | 1.85x | 1.7x | 1.5x | 1.3-1.5x |

## 验证方案

每个优化完成后：

```bash
# 先跑受影响模块的定向测试
cargo test -p flow-core --test pipelines
cargo test -p flow-core --test scalable_data_pipelines

# 功能测试
cargo test --workspace

# 性能验证
cargo run --release -p benchmarks --bin taskflow_compare -- \
  --threads 4 --rounds 3 \
  --cases fibonacci,thread_pool,data_pipeline,linear_pipeline,graph_pipeline,wavefront

# 安全检查（nightly）
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test -p flow-core
```

额外执行约束：

- `A`、`B`、`D` 可以直接进主线，但每项都要有单独 benchmark 记录与回滚点
- `C3` 和 `E` 必须在独立分支验证，通过后再决定是否合入主线
- 若某项优化让 `embarrassing_par`、`scan`、`binary_tree` 这类已改善 case 回退超过 3%，默认回滚
- 对 `E`，除了 benchmark，还必须补充针对 `corun_children` 的竞态/压力测试，否则不合入
