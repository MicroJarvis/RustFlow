use std::any::Any;
use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

use crate::async_handle::{AsyncHandle, AsyncState};
use crate::error::FlowError;
use crate::flow::{Flow, GraphSnapshot, NodeSnapshot, Subflow, TaskId, TaskKind};
use crate::observer::Observer;
use crate::runtime::RuntimeCtx;

const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct RunHandle {
    shared: Arc<RunHandleState>,
}

impl RunHandle {
    pub(crate) fn pending() -> Self {
        Self {
            shared: Arc::new(RunHandleState::default()),
        }
    }

    pub(crate) fn complete(&self, result: Result<(), FlowError>) {
        self.shared.complete(result);
    }

    pub fn wait(&self) -> Result<(), FlowError> {
        self.shared.wait()
    }

    pub fn is_finished(&self) -> bool {
        self.shared.is_finished()
    }

    pub fn cancel(&self) -> bool {
        self.shared.cancel()
    }

    pub(crate) fn cancelled_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shared.cancelled)
    }
}

struct RunHandleState {
    result: Mutex<Option<Result<(), FlowError>>>,
    ready: Condvar,
    cancelled: Arc<AtomicBool>,
}

impl Default for RunHandleState {
    fn default() -> Self {
        Self {
            result: Mutex::new(None),
            ready: Condvar::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl RunHandleState {
    fn complete(&self, result: Result<(), FlowError>) {
        let mut slot = self.result.lock().expect("run handle state poisoned");
        if slot.is_none() {
            *slot = Some(result);
            self.ready.notify_all();
        }
    }

    fn wait(&self) -> Result<(), FlowError> {
        let mut slot = self.result.lock().expect("run handle state poisoned");
        while slot.is_none() {
            slot = self.ready.wait(slot).expect("run handle wait poisoned");
        }
        slot.clone()
            .expect("run handle result must be available after wait")
    }

    fn is_finished(&self) -> bool {
        self.result
            .lock()
            .expect("run handle state poisoned")
            .is_some()
    }

    fn cancel(&self) -> bool {
        let slot = self.result.lock().expect("run handle state poisoned");
        if slot.is_some() {
            return false;
        }
        drop(slot);

        self.cancelled.store(true, Ordering::Release);
        true
    }
}

#[derive(Clone)]
pub struct Executor {
    inner: Arc<ExecutorInner>,
}

impl Executor {
    pub fn new(worker_count: usize) -> Self {
        assert!(worker_count > 0, "executor must have at least one worker");

        let workers = (0..worker_count)
            .map(|_| Arc::new(WorkerQueue::default()))
            .collect();

        let inner = Arc::new(ExecutorInner {
            workers,
            notifier: Notifier::default(),
            worker_count,
            next_worker: AtomicUsize::new(0),
            active_runs: Mutex::new(0),
            active_runs_ready: Condvar::new(),
            shutdown: AtomicBool::new(false),
            threads: Mutex::new(Vec::with_capacity(worker_count)),
            observers: RwLock::new(Vec::new()),
        });

        let mut threads = inner.threads.lock().expect("executor threads poisoned");
        for worker_id in 0..worker_count {
            let inner_clone = Arc::clone(&inner);
            let thread = thread::Builder::new()
                .name(format!("rustflow-worker-{worker_id}"))
                .stack_size(WORKER_STACK_SIZE)
                .spawn(move || worker_loop(worker_id, inner_clone))
                .expect("failed to spawn executor worker thread");
            threads.push(thread);
        }
        drop(threads);

        Self { inner }
    }

    pub fn num_workers(&self) -> usize {
        self.inner.worker_count
    }

    pub fn run(&self, flow: &Flow) -> RunHandle {
        self.start_run(Arc::new(flow.snapshot()), None)
    }

    pub fn run_n(&self, flow: &Flow, n: usize) -> RunHandle {
        let handle = RunHandle::pending();
        if n == 0 {
            handle.complete(Ok(()));
            return handle;
        }

        let executor = self.clone();
        let flow = flow.clone();
        let handle_clone = handle.clone();

        thread::spawn(move || {
            let result = (|| {
                for _ in 0..n {
                    executor.run(&flow).wait()?;
                }
                Ok(())
            })();
            handle_clone.complete(result);
        });

        handle
    }

    pub fn run_until<P>(&self, flow: &Flow, mut predicate: P) -> RunHandle
    where
        P: FnMut() -> bool + Send + 'static,
    {
        let handle = RunHandle::pending();
        let executor = self.clone();
        let flow = flow.clone();
        let handle_clone = handle.clone();

        thread::spawn(move || {
            let result = (|| loop {
                executor.run(&flow).wait()?;
                if predicate() {
                    return Ok(());
                }
            })();
            handle_clone.complete(result);
        });

        handle
    }

    pub fn wait_for_all(&self) {
        let mut active_runs = self
            .inner
            .active_runs
            .lock()
            .expect("executor active runs poisoned");
        while *active_runs > 0 {
            active_runs = self
                .inner
                .active_runs_ready
                .wait(active_runs)
                .expect("executor wait_for_all poisoned");
        }
    }

    pub fn add_observer(&self, observer: Arc<dyn Observer>) {
        observer.set_up(self.num_workers());
        self.inner
            .observers
            .write()
            .expect("executor observers poisoned")
            .push(observer);
    }

    pub fn async_task<F, T>(&self, task: F) -> AsyncHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let state = Arc::new(AsyncState::new());
        let run_handle = self.schedule_async_runner(
            Box::new({
                let state = Arc::clone(&state);
                move |_| {
                    state.store(task());
                    Ok(())
                }
            }),
            None,
        );

        AsyncHandle::new(run_handle, state)
    }

    pub fn silent_async<F>(&self, task: F) -> RunHandle
    where
        F: FnOnce() + Send + 'static,
    {
        self.schedule_async_runner(
            Box::new(move |_| {
                task();
                Ok(())
            }),
            None,
        )
    }

    pub fn runtime_async<F, T>(&self, task: F) -> AsyncHandle<T>
    where
        F: FnOnce(&RuntimeCtx) -> T + Send + 'static,
        T: Send + 'static,
    {
        let state = Arc::new(AsyncState::new());
        let run_handle = self.schedule_async_runner(
            Box::new({
                let state = Arc::clone(&state);
                move |runtime| {
                    state.store(task(runtime));
                    Ok(())
                }
            }),
            None,
        );

        AsyncHandle::new(run_handle, state)
    }

    pub fn runtime_silent_async<F>(&self, task: F) -> RunHandle
    where
        F: FnOnce(&RuntimeCtx) + Send + 'static,
    {
        self.schedule_async_runner(
            Box::new(move |runtime| {
                task(runtime);
                Ok(())
            }),
            None,
        )
    }

    pub(crate) fn corun_inline(&self, flow: &Flow, worker_id: usize) -> Result<(), FlowError> {
        let handle = self.start_run(Arc::new(flow.snapshot()), Some(worker_id));
        self.wait_handle_inline(&handle, worker_id)
    }

    pub(crate) fn wait_handle_inline(
        &self,
        handle: &RunHandle,
        worker_id: usize,
    ) -> Result<(), FlowError> {
        self.wait_handles_inline(std::slice::from_ref(handle), worker_id)
    }

    pub(crate) fn wait_handles_inline(
        &self,
        handles: &[RunHandle],
        worker_id: usize,
    ) -> Result<(), FlowError> {
        let mut known_epoch = self.inner.notifier.current_epoch();

        while handles.iter().any(|handle| !handle.is_finished()) {
            if let Some(task) = self.inner.next_task(worker_id) {
                self.inner.execute(self, worker_id, task);
                continue;
            }

            if handles.iter().all(RunHandle::is_finished) {
                break;
            }

            self.inner
                .notifier
                .wait(&mut known_epoch, &self.inner.shutdown);
        }

        for handle in handles {
            handle.wait()?;
        }

        Ok(())
    }

    fn start_run(&self, snapshot: Arc<GraphSnapshot>, worker_hint: Option<usize>) -> RunHandle {
        let handle = RunHandle::pending();

        if snapshot.nodes.is_empty() {
            handle.complete(Ok(()));
            return handle;
        }

        self.inner.increment_active_runs();

        let run = Arc::new(RunState::new(snapshot, handle.clone()));
        let root_indices = run.root_indices();

        if root_indices.is_empty() {
            self.inner.decrement_active_runs();
            handle.complete(Ok(()));
            return handle;
        }

        run.outstanding
            .fetch_add(root_indices.len(), Ordering::AcqRel);

        for task_index in root_indices {
            self.inner.schedule_precounted(
                ScheduledTask {
                    run: Arc::clone(&run),
                    task_index,
                },
                worker_hint,
            );
        }

        handle
    }

    pub(crate) fn schedule_async_runner(
        &self,
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        worker_hint: Option<usize>,
    ) -> RunHandle {
        let handle = RunHandle::pending();
        self.inner.increment_active_runs();
        self.inner.enqueue_async(
            ScheduledAsyncTask::new(runner, handle.clone()),
            worker_hint,
        );
        handle
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) != 1 {
            return;
        }

        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.notifier.notify_all();

        let mut threads = self
            .inner
            .threads
            .lock()
            .expect("executor threads poisoned");
        for thread in threads.drain(..) {
            let _ = thread.join();
        }
    }
}

#[derive(Default)]
struct WorkerQueue {
    tasks: Mutex<VecDeque<ScheduledWork>>,
}

impl WorkerQueue {
    fn push_back(&self, task: ScheduledWork) {
        self.tasks
            .lock()
            .expect("worker queue poisoned")
            .push_back(task);
    }

    fn pop_back(&self) -> Option<ScheduledWork> {
        self.tasks.lock().expect("worker queue poisoned").pop_back()
    }

    fn steal_front(&self) -> Option<ScheduledWork> {
        self.tasks
            .lock()
            .expect("worker queue poisoned")
            .pop_front()
    }
}

#[derive(Default)]
struct Notifier {
    epoch: Mutex<u64>,
    ready: Condvar,
}

impl Notifier {
    fn current_epoch(&self) -> u64 {
        *self.epoch.lock().expect("notifier poisoned")
    }

    fn notify_all(&self) {
        let mut epoch = self.epoch.lock().expect("notifier poisoned");
        *epoch += 1;
        self.ready.notify_all();
    }

    fn wait(&self, known_epoch: &mut u64, shutdown: &AtomicBool) {
        let mut epoch = self.epoch.lock().expect("notifier poisoned");
        while *epoch == *known_epoch && !shutdown.load(Ordering::Acquire) {
            epoch = self.ready.wait(epoch).expect("notifier wait poisoned");
        }
        *known_epoch = *epoch;
    }
}

struct ExecutorInner {
    workers: Vec<Arc<WorkerQueue>>,
    notifier: Notifier,
    worker_count: usize,
    next_worker: AtomicUsize,
    active_runs: Mutex<usize>,
    active_runs_ready: Condvar,
    shutdown: AtomicBool,
    threads: Mutex<Vec<JoinHandle<()>>>,
    observers: RwLock<Vec<Arc<dyn Observer>>>,
}

impl ExecutorInner {
    fn increment_active_runs(&self) {
        let mut active_runs = self
            .active_runs
            .lock()
            .expect("executor active runs poisoned");
        *active_runs += 1;
    }

    fn decrement_active_runs(&self) {
        let mut active_runs = self
            .active_runs
            .lock()
            .expect("executor active runs poisoned");
        *active_runs = active_runs.saturating_sub(1);
        if *active_runs == 0 {
            self.active_runs_ready.notify_all();
        }
    }

    fn notify_task_started(&self, worker_id: usize, node: &NodeSnapshot) {
        let observers = self.observers.read().expect("executor observers poisoned");
        for observer in observers.iter() {
            observer.task_started(worker_id, node.id, node.name.as_deref());
        }
    }

    fn notify_task_finished(&self, worker_id: usize, node: &NodeSnapshot) {
        let observers = self.observers.read().expect("executor observers poisoned");
        for observer in observers.iter() {
            observer.task_finished(worker_id, node.id, node.name.as_deref());
        }
    }

    fn notify_worker_sleep(&self, worker_id: usize) {
        let observers = self.observers.read().expect("executor observers poisoned");
        for observer in observers.iter() {
            observer.worker_sleep(worker_id);
        }
    }

    fn notify_worker_wake(&self, worker_id: usize) {
        let observers = self.observers.read().expect("executor observers poisoned");
        for observer in observers.iter() {
            observer.worker_wake(worker_id);
        }
    }

    fn schedule(&self, task: ScheduledTask, worker_hint: Option<usize>) {
        task.run.outstanding.fetch_add(1, Ordering::AcqRel);
        self.enqueue_graph(task, worker_hint);
    }

    fn schedule_precounted(&self, task: ScheduledTask, worker_hint: Option<usize>) {
        self.enqueue_graph(task, worker_hint);
    }

    fn enqueue_graph(&self, task: ScheduledTask, worker_hint: Option<usize>) {
        self.enqueue(ScheduledWork::Graph(task), worker_hint);
    }

    fn enqueue_async(&self, task: ScheduledAsyncTask, worker_hint: Option<usize>) {
        self.enqueue(ScheduledWork::Async(task), worker_hint);
    }

    fn enqueue(&self, task: ScheduledWork, worker_hint: Option<usize>) {
        let worker_id = worker_hint.unwrap_or_else(|| {
            self.next_worker.fetch_add(1, Ordering::Relaxed) % self.worker_count
        });
        self.workers[worker_id].push_back(task);
        self.notifier.notify_all();
    }

    fn reschedule_waiters(&self, waiters: Vec<ScheduledTask>, worker_hint: Option<usize>) {
        if waiters.is_empty() {
            return;
        }

        for waiter in waiters {
            let worker_id = worker_hint.unwrap_or_else(|| {
                self.next_worker.fetch_add(1, Ordering::Relaxed) % self.worker_count
            });
            self.workers[worker_id].push_back(ScheduledWork::Graph(waiter));
        }

        self.notifier.notify_all();
    }

    fn next_task(&self, worker_id: usize) -> Option<ScheduledWork> {
        if let Some(task) = self.workers[worker_id].pop_back() {
            return Some(task);
        }

        for offset in 1..=self.worker_count {
            let victim = (worker_id + offset) % self.worker_count;
            if victim == worker_id {
                continue;
            }
            if let Some(task) = self.workers[victim].steal_front() {
                return Some(task);
            }
        }

        None
    }

    fn execute(&self, executor: &Executor, worker_id: usize, scheduled: ScheduledWork) {
        match scheduled {
            ScheduledWork::Graph(task) => self.execute_graph(executor, worker_id, task),
            ScheduledWork::Async(task) => self.execute_async(executor, worker_id, task),
        }
    }

    fn execute_graph(&self, executor: &Executor, worker_id: usize, scheduled: ScheduledTask) {
        let node = &scheduled.run.graph.nodes[scheduled.task_index];

        if scheduled.run.is_cancelled() {
            self.finish_task(&scheduled);
            return;
        }

        if !self.acquire_all(node, &scheduled, Some(worker_id)) {
            return;
        }

        let run = Arc::clone(&scheduled.run);
        let executor_for_scheduler = executor.clone();
        let scheduler = Arc::new(move |task_id: TaskId| {
            executor_for_scheduler.inner.schedule(
                ScheduledTask {
                    run: Arc::clone(&run),
                    task_index: task_id.index(),
                },
                Some(worker_id),
            );
        });

        self.notify_task_started(worker_id, node);
        let selected_successors = execute_node_inline(
            node,
            executor,
            worker_id,
            Some(scheduler),
            Arc::clone(&scheduled.run.cancelled),
        );
        self.notify_task_finished(worker_id, node);

        if let Err(error) = &selected_successors {
            scheduled.run.record_error(error.clone());
        }

        self.release_all(node, Some(worker_id));

        if scheduled.run.is_cancelled() {
            self.finish_task(&scheduled);
            return;
        }

        if node.kind.is_condition() {
            if let Ok(Some(successors)) = selected_successors {
                for successor_position in successors {
                    if let Some(&successor) = node.successors.get(successor_position) {
                        self.schedule(
                            ScheduledTask {
                                run: Arc::clone(&scheduled.run),
                                task_index: successor,
                            },
                            Some(worker_id),
                        );
                    }
                }
            }
        } else if selected_successors.is_ok() {
            for &successor in &node.successors {
                let previous = scheduled.run.pending[successor].fetch_sub(1, Ordering::AcqRel);
                debug_assert!(previous > 0, "successor pending count underflow");
                if previous == 1 {
                    self.schedule(
                        ScheduledTask {
                            run: Arc::clone(&scheduled.run),
                            task_index: successor,
                        },
                        Some(worker_id),
                    );
                }
            }
        }

        self.finish_task(&scheduled);
    }

    fn execute_async(&self, executor: &Executor, worker_id: usize, mut scheduled: ScheduledAsyncTask) {
        if scheduled
            .run_handle
            .cancelled_flag()
            .load(Ordering::Acquire)
        {
            scheduled.run_handle.complete(Ok(()));
            self.decrement_active_runs();
            self.notifier.notify_all();
            return;
        }

        let runner = scheduled
            .runner
            .take()
            .expect("async runner scheduled more than once");
        let cancelled = scheduled.run_handle.cancelled_flag();
        let runtime = RuntimeCtx::new(executor.clone(), worker_id, None, cancelled);
        let result = match panic::catch_unwind(AssertUnwindSafe(|| runner(&runtime))) {
            Ok(result) => result,
            Err(payload) => Err(FlowError::plain(panic_payload_to_string(payload))),
        };

        scheduled.run_handle.complete(result);
        self.decrement_active_runs();
        self.notifier.notify_all();
    }

    fn finish_task(&self, scheduled: &ScheduledTask) {
        let node = &scheduled.run.graph.nodes[scheduled.task_index];
        scheduled.run.pending[scheduled.task_index]
            .store(node.predecessor_count, Ordering::Release);

        if scheduled.run.outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
            let result = scheduled.run.final_result();
            scheduled.run.handle.complete(result);
            self.decrement_active_runs();
            self.notifier.notify_all();
        }
    }

    fn acquire_all(
        &self,
        node: &NodeSnapshot,
        scheduled: &ScheduledTask,
        worker_hint: Option<usize>,
    ) -> bool {
        let mut acquired = Vec::new();

        for semaphore in &node.to_acquire {
            if semaphore.try_acquire_or_wait(scheduled.clone()) {
                acquired.push(semaphore.clone());
                continue;
            }

            let mut waiters = Vec::new();
            for semaphore in acquired.into_iter().rev() {
                semaphore.release(&mut waiters);
            }
            self.reschedule_waiters(waiters, worker_hint);
            return false;
        }

        true
    }

    fn release_all(&self, node: &NodeSnapshot, worker_hint: Option<usize>) {
        let mut waiters = Vec::new();
        for semaphore in &node.to_release {
            semaphore.release(&mut waiters);
        }
        self.reschedule_waiters(waiters, worker_hint);
    }
}

#[derive(Clone)]
pub(crate) struct ScheduledTask {
    run: Arc<RunState>,
    task_index: usize,
}

enum ScheduledWork {
    Graph(ScheduledTask),
    Async(ScheduledAsyncTask),
}

struct ScheduledAsyncTask {
    run_handle: RunHandle,
    runner: Option<Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>>,
}

impl ScheduledAsyncTask {
    fn new(
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        run_handle: RunHandle,
    ) -> Self {
        Self {
            run_handle,
            runner: Some(runner),
        }
    }
}

struct RunState {
    graph: Arc<GraphSnapshot>,
    pending: Vec<AtomicUsize>,
    outstanding: AtomicUsize,
    error: Mutex<Option<FlowError>>,
    handle: RunHandle,
    cancelled: Arc<AtomicBool>,
}

impl RunState {
    fn new(graph: Arc<GraphSnapshot>, handle: RunHandle) -> Self {
        let pending = graph
            .nodes
            .iter()
            .map(|node| AtomicUsize::new(node.predecessor_count))
            .collect();

        Self {
            graph,
            pending,
            outstanding: AtomicUsize::new(0),
            error: Mutex::new(None),
            cancelled: handle.cancelled_flag(),
            handle,
        }
    }

    fn root_indices(&self) -> Vec<usize> {
        self.graph
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (node.total_predecessor_count == 0).then_some(index))
            .collect()
    }

    fn record_error(&self, error: FlowError) {
        let mut slot = self.error.lock().expect("run state error poisoned");
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    fn final_result(&self) -> Result<(), FlowError> {
        self.error
            .lock()
            .expect("run state error poisoned")
            .clone()
            .map_or(Ok(()), Err)
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

fn worker_loop(worker_id: usize, inner: Arc<ExecutorInner>) {
    let executor = Executor {
        inner: Arc::clone(&inner),
    };
    let mut known_epoch = inner.notifier.current_epoch();

    while !inner.shutdown.load(Ordering::Acquire) {
        if let Some(task) = inner.next_task(worker_id) {
            inner.execute(&executor, worker_id, task);
            continue;
        }

        inner.notify_worker_sleep(worker_id);
        inner.notifier.wait(&mut known_epoch, &inner.shutdown);
        if !inner.shutdown.load(Ordering::Acquire) {
            inner.notify_worker_wake(worker_id);
        }
    }
}

pub(crate) fn panic_payload_to_string(payload: Box<dyn Any + Send + 'static>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "task panicked with a non-string payload".to_string(),
        },
    }
}

fn execute_node_inline(
    node: &NodeSnapshot,
    executor: &Executor,
    worker_id: usize,
    scheduler: Option<Arc<dyn Fn(TaskId) + Send + Sync + 'static>>,
    cancelled: Arc<AtomicBool>,
) -> Result<Option<Vec<usize>>, FlowError> {
    match &node.kind {
        TaskKind::Placeholder => Ok(None),
        TaskKind::Static(task) => match panic::catch_unwind(AssertUnwindSafe(|| task())) {
            Ok(()) => Ok(None),
            Err(payload) => Err(FlowError::panic(
                node.id,
                node.name.as_deref(),
                panic_payload_to_string(payload),
            )),
        },
        TaskKind::Runtime(task) => {
            let runtime = RuntimeCtx::new(executor.clone(), worker_id, scheduler, cancelled);
            match panic::catch_unwind(AssertUnwindSafe(|| task(&runtime))) {
                Ok(()) => Ok(None),
                Err(payload) => Err(FlowError::panic(
                    node.id,
                    node.name.as_deref(),
                    panic_payload_to_string(payload),
                )),
            }
        }
        TaskKind::Condition(task) => match panic::catch_unwind(AssertUnwindSafe(|| task())) {
            Ok(index) => Ok(Some(vec![index])),
            Err(payload) => Err(FlowError::panic(
                node.id,
                node.name.as_deref(),
                panic_payload_to_string(payload),
            )),
        },
        TaskKind::MultiCondition(task) => match panic::catch_unwind(AssertUnwindSafe(|| task())) {
            Ok(indices) => Ok(Some(indices)),
            Err(payload) => Err(FlowError::panic(
                node.id,
                node.name.as_deref(),
                panic_payload_to_string(payload),
            )),
        },
        TaskKind::Subflow(task) => {
            let subflow = Subflow::new();
            match panic::catch_unwind(AssertUnwindSafe(|| task(&subflow))) {
                Ok(()) => {
                    let snapshot = subflow.snapshot();
                    execute_graph_inline(&snapshot, executor, worker_id, Arc::clone(&cancelled))?;
                    Ok(None)
                }
                Err(payload) => Err(FlowError::panic(
                    node.id,
                    node.name.as_deref(),
                    panic_payload_to_string(payload),
                )),
            }
        }
        TaskKind::Composed(flow) => {
            let snapshot = flow.snapshot();
            execute_graph_inline(&snapshot, executor, worker_id, cancelled)?;
            Ok(None)
        }
    }
}

fn execute_graph_inline(
    graph: &GraphSnapshot,
    executor: &Executor,
    worker_id: usize,
    cancelled: Arc<AtomicBool>,
) -> Result<(), FlowError> {
    if graph.nodes.is_empty() {
        return Ok(());
    }

    let mut pending: Vec<usize> = graph
        .nodes
        .iter()
        .map(|node| node.predecessor_count)
        .collect();
    let mut queue: VecDeque<usize> = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| (node.total_predecessor_count == 0).then_some(index))
        .collect();
    let mut outstanding = queue.len();

    while let Some(task_index) = queue.pop_front() {
        let node = &graph.nodes[task_index];
        if cancelled.load(Ordering::Acquire) {
            pending[task_index] = node.predecessor_count;
            outstanding -= 1;
            if outstanding == 0 {
                break;
            }
            continue;
        }

        executor.inner.notify_task_started(worker_id, node);
        let selected_successors =
            execute_node_inline(node, executor, worker_id, None, Arc::clone(&cancelled));
        executor.inner.notify_task_finished(worker_id, node);
        let selected_successors = selected_successors?;

        if cancelled.load(Ordering::Acquire) {
            pending[task_index] = node.predecessor_count;
            outstanding -= 1;
            if outstanding == 0 {
                break;
            }
            continue;
        }

        if node.kind.is_condition() {
            if let Some(successors) = selected_successors {
                for successor_position in successors {
                    if let Some(&successor) = node.successors.get(successor_position) {
                        queue.push_back(successor);
                        outstanding += 1;
                    }
                }
            }
        } else {
            for &successor in &node.successors {
                let previous = pending[successor];
                debug_assert!(previous > 0, "successor pending count underflow");
                pending[successor] = previous - 1;
                if previous == 1 {
                    queue.push_back(successor);
                    outstanding += 1;
                }
            }
        }

        pending[task_index] = node.predecessor_count;
        outstanding -= 1;
        if outstanding == 0 {
            break;
        }
    }

    Ok(())
}
