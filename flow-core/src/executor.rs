use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{fence, AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};

use crate::async_handle::{AsyncHandle, AsyncState, RuntimeAsyncState};
use crate::error::FlowError;
use crate::flow::{Flow, GraphSnapshot, NodeSnapshot, Subflow, TaskId, TaskKind};
use crate::observer::Observer;
use crate::runtime::{RuntimeCtx, RuntimeJoinScope};

const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone)]
struct RuntimeSpawnContext {
    executor: Arc<ExecutorInner>,
    worker_id: usize,
    cancelled: Arc<AtomicBool>,
    join_scope: Arc<RuntimeJoinScope>,
}

thread_local! {
    static CURRENT_RUNTIME_SPAWN_CONTEXT: RefCell<Option<RuntimeSpawnContext>> = RefCell::new(None);
}

struct RuntimeSpawnContextGuard {
    previous: Option<RuntimeSpawnContext>,
}

impl Drop for RuntimeSpawnContextGuard {
    fn drop(&mut self) {
        CURRENT_RUNTIME_SPAWN_CONTEXT.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

fn install_runtime_spawn_context(context: RuntimeSpawnContext) -> RuntimeSpawnContextGuard {
    let previous = CURRENT_RUNTIME_SPAWN_CONTEXT.with(|slot| slot.replace(Some(context)));
    RuntimeSpawnContextGuard { previous }
}

#[derive(Clone, Copy)]
enum DriveMode {
    AllowPark,
    NoPark,
}

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
    result: OnceLock<Result<(), FlowError>>,
    waiter: OnceLock<Box<RunHandleWaiter>>,
    finished: AtomicBool,
    cancelled: Arc<AtomicBool>,
}

impl Default for RunHandleState {
    fn default() -> Self {
        Self {
            result: OnceLock::new(),
            waiter: OnceLock::new(),
            finished: AtomicBool::new(false),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Default)]
struct RunHandleWaiter {
    lock: Mutex<()>,
    ready: Condvar,
}

impl RunHandleState {
    fn complete(&self, result: Result<(), FlowError>) {
        if let Some(waiter) = self.waiter.get() {
            let _guard = waiter.lock.lock().expect("run handle wait poisoned");
            if self.result.set(result).is_ok() {
                self.finished.store(true, Ordering::Release);
                waiter.ready.notify_all();
            }
        } else if self.result.set(result).is_ok() {
            self.finished.store(true, Ordering::Release);
        }
    }

    fn wait(&self) -> Result<(), FlowError> {
        if self.finished.load(Ordering::Acquire) {
            return self
                .result
                .get()
                .expect("run handle result must be available after completion")
                .clone();
        }

        let waiter = self
            .waiter
            .get_or_init(|| Box::new(RunHandleWaiter::default()));
        let mut guard = waiter.lock.lock().expect("run handle wait poisoned");
        while !self.finished.load(Ordering::Acquire) {
            guard = waiter.ready.wait(guard).expect("run handle wait poisoned");
        }
        drop(guard);
        self.result
            .get()
            .expect("run handle result must be available after wait")
            .clone()
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    fn cancel(&self) -> bool {
        if self.finished.load(Ordering::Acquire) {
            return false;
        }

        if self.result.get().is_some() {
            return false;
        }

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
        let parkers = (0..worker_count)
            .map(|_| Arc::new(WorkerParker::default()))
            .collect();

        let inner = Arc::new(ExecutorInner {
            workers,
            parkers,
            run_completion: RunCompletionEvent::default(),
            worker_count,
            next_worker: AtomicUsize::new(0),
            active_runs: AtomicUsize::new(0),
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
        let mut known_epoch = self.inner.run_completion.current_epoch();
        while self.inner.active_runs.load(Ordering::Acquire) > 0 {
            self.inner
                .run_completion
                .wait(&mut known_epoch, &self.inner.shutdown);
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
        if let Some(context) = self.current_runtime_spawn_context() {
            let state = Arc::new(RuntimeAsyncState::new());
            let value_state = Arc::clone(&state);
            let error_state = Arc::clone(&state);
            self.inner.increment_active_runs();
            self.inner.enqueue_async(
                ScheduledAsyncTask::runtime_value(
                    Box::new(move |runtime| {
                        value_state.complete_success(task(runtime));
                        Ok(())
                    }),
                    context.cancelled,
                    Box::new(move |error| error_state.complete_error(error)),
                ),
                Some(context.worker_id),
            );

            return AsyncHandle::from_runtime_state(state);
        }

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
        let context = self.current_runtime_spawn_context();
        self.schedule_async_runner_with_runtime_cancelled(
            Box::new(move |runtime| {
                task(runtime);
                Ok(())
            }),
            context.as_ref().map(|context| context.worker_id),
            context
                .as_ref()
                .map(|context| Arc::clone(&context.cancelled)),
            context
                .as_ref()
                .map(|context| Arc::clone(&context.join_scope)),
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
        self.wait_until_inline(worker_id, || handles.iter().all(RunHandle::is_finished));

        for handle in handles {
            handle.wait()?;
        }

        Ok(())
    }

    pub(crate) fn wait_until_inline<F>(&self, worker_id: usize, mut done: F)
    where
        F: FnMut() -> bool,
    {
        self.drive_worker_until(worker_id, DriveMode::NoPark, &mut done);
    }

    pub(crate) fn schedule_runtime_runner(
        &self,
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
    ) -> RunHandle {
        self.schedule_async_runner(runner, None)
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

        let scheduled_roots = root_indices
            .into_iter()
            .map(|task_index| ScheduledTask {
                run: Arc::clone(&run),
                task_index,
            })
            .collect();
        self.inner
            .schedule_precounted_batch(scheduled_roots, worker_hint);

        handle
    }

    pub(crate) fn schedule_async_runner(
        &self,
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        worker_hint: Option<usize>,
    ) -> RunHandle {
        self.schedule_async_runner_with_runtime_cancelled(runner, worker_hint, None, None)
    }

    pub(crate) fn schedule_runtime_silent_child(
        &self,
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        worker_hint: usize,
        runtime_cancelled: Arc<AtomicBool>,
        join_scope: Arc<RuntimeJoinScope>,
    ) {
        join_scope.add_child();
        self.inner.increment_active_runs();
        self.inner.enqueue_async(
            ScheduledAsyncTask::runtime_child(runner, runtime_cancelled, join_scope),
            Some(worker_hint),
        );
    }

    fn current_runtime_spawn_context(&self) -> Option<RuntimeSpawnContext> {
        CURRENT_RUNTIME_SPAWN_CONTEXT.with(|slot| {
            let context = slot.borrow().clone()?;
            Arc::ptr_eq(&context.executor, &self.inner).then_some(context)
        })
    }

    fn schedule_async_runner_with_runtime_cancelled(
        &self,
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        worker_hint: Option<usize>,
        runtime_cancelled: Option<Arc<AtomicBool>>,
        join_scope: Option<Arc<RuntimeJoinScope>>,
    ) -> RunHandle {
        let handle = RunHandle::pending();
        self.inner.increment_active_runs();
        let runtime_cancelled = runtime_cancelled.unwrap_or_else(|| handle.cancelled_flag());
        if let Some(join_scope) = &join_scope {
            join_scope.add_child();
        }
        self.inner.enqueue_async(
            ScheduledAsyncTask::run_handle(runner, handle.clone(), runtime_cancelled, join_scope),
            worker_hint,
        );
        handle
    }

    fn drive_worker_until<F>(&self, worker_id: usize, mode: DriveMode, done: &mut F)
    where
        F: FnMut() -> bool,
    {
        let mut continuation = None;

        loop {
            if done() {
                return;
            }

            let task = continuation
                .take()
                .or_else(|| self.inner.next_task(worker_id));
            if let Some(task) = task {
                continuation = self.inner.execute(self, worker_id, task);
                continue;
            }

            match mode {
                DriveMode::AllowPark => {
                    let observed_epoch = self.inner.prepare_park(worker_id);

                    if done() {
                        self.inner.cancel_park(worker_id, observed_epoch);
                        return;
                    }

                    let task = self.inner.next_task(worker_id);
                    if let Some(task) = task {
                        self.inner.cancel_park(worker_id, observed_epoch);
                        continuation = self.inner.execute(self, worker_id, task);
                        continue;
                    }

                    self.inner
                        .commit_park(worker_id, observed_epoch, &self.inner.shutdown);
                }
                DriveMode::NoPark => thread::yield_now(),
            }
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) != 1 {
            return;
        }

        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.wake_all_workers();
        self.inner.run_completion.notify_all();

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
    fn push_back(&self, task: ScheduledWork) -> bool {
        let mut tasks = self.tasks.lock().expect("worker queue poisoned");
        let was_empty = tasks.is_empty();
        tasks.push_back(task);
        was_empty
    }

    fn push_back_many(&self, tasks_to_push: Vec<ScheduledWork>) -> bool {
        let mut tasks = self.tasks.lock().expect("worker queue poisoned");
        let was_empty = tasks.is_empty();
        tasks.extend(tasks_to_push);
        was_empty
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
struct WorkerParker {
    state: AtomicU8,
    epoch: AtomicU32,
    thread: OnceLock<thread::Thread>,
}

#[derive(Default)]
struct RunCompletionEvent {
    epoch: Mutex<u64>,
    ready: Condvar,
}

impl RunCompletionEvent {
    fn current_epoch(&self) -> u64 {
        *self.epoch.lock().expect("run completion poisoned")
    }

    fn notify_all(&self) {
        let mut epoch = self.epoch.lock().expect("run completion poisoned");
        *epoch += 1;
        self.ready.notify_all();
    }

    fn wait(&self, known_epoch: &mut u64, shutdown: &AtomicBool) {
        let mut epoch = self.epoch.lock().expect("run completion poisoned");
        while *epoch == *known_epoch && !shutdown.load(Ordering::Acquire) {
            epoch = self
                .ready
                .wait(epoch)
                .expect("run completion wait poisoned");
        }
        *known_epoch = *epoch;
    }
}

struct ExecutorInner {
    workers: Vec<Arc<WorkerQueue>>,
    parkers: Vec<Arc<WorkerParker>>,
    run_completion: RunCompletionEvent,
    worker_count: usize,
    next_worker: AtomicUsize,
    active_runs: AtomicUsize,
    shutdown: AtomicBool,
    threads: Mutex<Vec<JoinHandle<()>>>,
    observers: RwLock<Vec<Arc<dyn Observer>>>,
}

const WORKER_RUNNING: u8 = 0;
const WORKER_PREPARING: u8 = 1;
const WORKER_PARKED: u8 = 2;

impl ExecutorInner {
    fn register_worker_thread(&self, worker_id: usize) {
        let _ = self.parkers[worker_id].thread.set(thread::current());
    }

    fn increment_active_runs(&self) {
        self.active_runs.fetch_add(1, Ordering::AcqRel);
    }

    fn decrement_active_runs(&self) {
        let previous = self.active_runs.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "executor active run count underflow");
        if previous == 1 {
            self.run_completion.notify_all();
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

    fn prepare_park(&self, worker_id: usize) -> u64 {
        let parker = &self.parkers[worker_id];
        parker.state.store(WORKER_PREPARING, Ordering::Relaxed);
        let epoch = parker.epoch.load(Ordering::Relaxed);
        fence(Ordering::SeqCst);
        u64::from(epoch)
    }

    fn cancel_park(&self, worker_id: usize, _observed_epoch: u64) {
        self.parkers[worker_id]
            .state
            .store(WORKER_RUNNING, Ordering::Relaxed);
    }

    fn commit_park(&self, worker_id: usize, observed_epoch: u64, shutdown: &AtomicBool) {
        let parker = &self.parkers[worker_id];
        let observed_epoch = observed_epoch as u32;

        if parker.state.load(Ordering::Relaxed) != WORKER_PREPARING
            || parker.epoch.load(Ordering::Relaxed) != observed_epoch
            || shutdown.load(Ordering::Acquire)
        {
            parker.state.store(WORKER_RUNNING, Ordering::Relaxed);
            return;
        }

        parker.state.store(WORKER_PARKED, Ordering::Release);
        self.notify_worker_sleep(worker_id);

        while parker.epoch.load(Ordering::Relaxed) == observed_epoch
            && !shutdown.load(Ordering::Acquire)
        {
            thread::park();
        }

        parker.state.store(WORKER_RUNNING, Ordering::Relaxed);
        if parker.epoch.load(Ordering::Relaxed) != observed_epoch
            && !shutdown.load(Ordering::Acquire)
        {
            self.notify_worker_wake(worker_id);
        }
    }

    fn wake_worker(&self, worker_id: usize) -> bool {
        let parker = &self.parkers[worker_id];
        let state = parker.state.load(Ordering::Acquire);
        if state != WORKER_PREPARING && state != WORKER_PARKED {
            return false;
        }

        parker.epoch.fetch_add(1, Ordering::Release);
        if let Some(thread) = parker.thread.get() {
            thread.unpark();
        }
        true
    }

    fn wake_all_workers(&self) {
        fence(Ordering::SeqCst);
        for worker_id in 0..self.parkers.len() {
            let _ = self.wake_worker(worker_id);
        }
    }

    fn schedule(&self, task: ScheduledTask, worker_hint: Option<usize>) {
        task.run.outstanding.fetch_add(1, Ordering::AcqRel);
        self.enqueue_graph(task, worker_hint);
    }

    fn schedule_precounted_batch(&self, tasks: Vec<ScheduledTask>, worker_hint: Option<usize>) {
        self.enqueue_many(
            tasks.into_iter().map(ScheduledWork::Graph).collect(),
            worker_hint,
        );
    }

    fn schedule_ready_graph(
        &self,
        continuation: &mut Option<ScheduledWork>,
        task: ScheduledTask,
        worker_hint: Option<usize>,
    ) {
        task.run.outstanding.fetch_add(1, Ordering::AcqRel);
        if continuation.is_none() {
            *continuation = Some(ScheduledWork::Graph(task));
        } else {
            self.enqueue_graph(task, worker_hint);
        }
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
        let queue_was_empty = self.workers[worker_id].push_back(task);
        if !queue_was_empty {
            return;
        }
        fence(Ordering::SeqCst);
        if !self.wake_worker(worker_id) {
            for parked_worker in 0..self.parkers.len() {
                if parked_worker != worker_id && self.wake_worker(parked_worker) {
                    break;
                }
            }
        }
    }

    fn enqueue_many(&self, tasks: Vec<ScheduledWork>, worker_hint: Option<usize>) {
        if tasks.is_empty() {
            return;
        }

        if let Some(worker_id) = worker_hint {
            let queue_was_empty = self.workers[worker_id].push_back_many(tasks);
            if !queue_was_empty {
                return;
            }
            fence(Ordering::SeqCst);
            if !self.wake_worker(worker_id) {
                for parked_worker in 0..self.parkers.len() {
                    if parked_worker != worker_id && self.wake_worker(parked_worker) {
                        break;
                    }
                }
            }
            return;
        }

        let start_worker = self.next_worker.fetch_add(tasks.len(), Ordering::Relaxed);
        let mut per_worker = (0..self.worker_count)
            .map(|_| Vec::new())
            .collect::<Vec<Vec<ScheduledWork>>>();

        for (offset, task) in tasks.into_iter().enumerate() {
            let worker_id = (start_worker + offset) % self.worker_count;
            per_worker[worker_id].push(task);
        }

        let mut activated_workers = Vec::new();
        for (worker_id, bucket) in per_worker.into_iter().enumerate() {
            if bucket.is_empty() {
                continue;
            }
            if self.workers[worker_id].push_back_many(bucket) {
                activated_workers.push(worker_id);
            }
        }

        if activated_workers.is_empty() {
            return;
        }

        fence(Ordering::SeqCst);
        for worker_id in activated_workers {
            let _ = self.wake_worker(worker_id);
        }
    }

    fn reschedule_waiters(&self, waiters: Vec<ScheduledTask>, worker_hint: Option<usize>) {
        if waiters.is_empty() {
            return;
        }
        self.enqueue_many(
            waiters.into_iter().map(ScheduledWork::Graph).collect(),
            worker_hint,
        );
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

    fn execute(
        &self,
        executor: &Executor,
        worker_id: usize,
        scheduled: ScheduledWork,
    ) -> Option<ScheduledWork> {
        match scheduled {
            ScheduledWork::Graph(task) => self.execute_graph(executor, worker_id, task),
            ScheduledWork::Async(task) => {
                self.execute_async(executor, worker_id, task);
                None
            }
        }
    }

    fn execute_graph(
        &self,
        executor: &Executor,
        worker_id: usize,
        scheduled: ScheduledTask,
    ) -> Option<ScheduledWork> {
        let node = &scheduled.run.graph.nodes[scheduled.task_index];
        let mut continuation = None;

        if scheduled.run.is_cancelled() {
            self.finish_task(&scheduled);
            return None;
        }

        if !self.acquire_all(node, &scheduled, Some(worker_id)) {
            return None;
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
            return None;
        }

        if node.kind.is_condition() {
            if let Ok(Some(successors)) = selected_successors {
                for successor_position in successors {
                    if let Some(&successor) = node.successors.get(successor_position) {
                        self.schedule_ready_graph(
                            &mut continuation,
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
                    self.schedule_ready_graph(
                        &mut continuation,
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
        continuation
    }

    fn execute_async(
        &self,
        executor: &Executor,
        worker_id: usize,
        mut scheduled: ScheduledAsyncTask,
    ) {
        let join_scope = scheduled.runtime_join_scope.take();
        if scheduled
            .skip_if_cancelled
            .as_ref()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
        {
            if let Some(on_success) = scheduled.on_success.take() {
                on_success();
            }
            if let Some(join_scope) = join_scope {
                join_scope.finish_child(Ok(()));
            }
            self.decrement_active_runs();
            return;
        }

        let runner = scheduled
            .runner
            .take()
            .expect("async runner scheduled more than once");
        let runtime_join_scope = Arc::new(RuntimeJoinScope::default());
        let runtime = RuntimeCtx::new(
            executor.clone(),
            worker_id,
            None,
            Arc::clone(&scheduled.runtime_cancelled),
            Arc::clone(&runtime_join_scope),
        );
        let _runtime_context = install_runtime_spawn_context(RuntimeSpawnContext {
            executor: Arc::clone(&executor.inner),
            worker_id,
            cancelled: Arc::clone(&scheduled.runtime_cancelled),
            join_scope: runtime_join_scope,
        });
        let result = match panic::catch_unwind(AssertUnwindSafe(|| runner(&runtime))) {
            Ok(result) => result,
            Err(payload) => Err(FlowError::plain(panic_payload_to_string(payload))),
        };

        let completion_result = match &result {
            Ok(()) => Ok(()),
            Err(error) => Err(error.clone()),
        };

        match result {
            Ok(()) => {
                if let Some(on_success) = scheduled.on_success.take() {
                    on_success();
                }
            }
            Err(error) => {
                if let Some(on_error) = scheduled.on_error.take() {
                    on_error(error.clone());
                }
            }
        }
        if let Some(join_scope) = join_scope {
            join_scope.finish_child(completion_result);
        }
        self.decrement_active_runs();
    }

    fn finish_task(&self, scheduled: &ScheduledTask) {
        let node = &scheduled.run.graph.nodes[scheduled.task_index];
        scheduled.run.pending[scheduled.task_index]
            .store(node.predecessor_count, Ordering::Release);

        if scheduled.run.outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
            let result = scheduled.run.final_result();
            scheduled.run.handle.complete(result);
            self.decrement_active_runs();
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
    runner: Option<Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>>,
    runtime_cancelled: Arc<AtomicBool>,
    runtime_join_scope: Option<Arc<RuntimeJoinScope>>,
    skip_if_cancelled: Option<Arc<AtomicBool>>,
    on_success: Option<Box<dyn FnOnce() + Send + 'static>>,
    on_error: Option<Box<dyn FnOnce(FlowError) + Send + 'static>>,
}

impl ScheduledAsyncTask {
    fn run_handle(
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        run_handle: RunHandle,
        runtime_cancelled: Arc<AtomicBool>,
        runtime_join_scope: Option<Arc<RuntimeJoinScope>>,
    ) -> Self {
        Self {
            runner: Some(runner),
            runtime_cancelled,
            runtime_join_scope,
            skip_if_cancelled: Some(run_handle.cancelled_flag()),
            on_success: Some(Box::new({
                let run_handle = run_handle.clone();
                move || run_handle.complete(Ok(()))
            })),
            on_error: Some(Box::new(move |error| run_handle.complete(Err(error)))),
        }
    }

    fn runtime_child(
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        runtime_cancelled: Arc<AtomicBool>,
        runtime_join_scope: Arc<RuntimeJoinScope>,
    ) -> Self {
        Self {
            runner: Some(runner),
            runtime_cancelled,
            runtime_join_scope: Some(runtime_join_scope),
            skip_if_cancelled: None,
            on_success: None,
            on_error: None,
        }
    }

    fn runtime_value(
        runner: Box<dyn FnOnce(&RuntimeCtx) -> Result<(), FlowError> + Send + 'static>,
        runtime_cancelled: Arc<AtomicBool>,
        on_error: Box<dyn FnOnce(FlowError) + Send + 'static>,
    ) -> Self {
        Self {
            runner: Some(runner),
            runtime_cancelled,
            runtime_join_scope: None,
            skip_if_cancelled: None,
            on_success: None,
            on_error: Some(on_error),
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
        self.graph.roots.clone()
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
    inner.register_worker_thread(worker_id);
    let executor = Executor {
        inner: Arc::clone(&inner),
    };
    let mut done = || inner.shutdown.load(Ordering::Acquire);
    executor.drive_worker_until(worker_id, DriveMode::AllowPark, &mut done);
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
            let runtime_join_scope = Arc::new(RuntimeJoinScope::default());
            let runtime = RuntimeCtx::new(
                executor.clone(),
                worker_id,
                scheduler,
                Arc::clone(&cancelled),
                Arc::clone(&runtime_join_scope),
            );
            let _runtime_context = install_runtime_spawn_context(RuntimeSpawnContext {
                executor: Arc::clone(&executor.inner),
                worker_id,
                cancelled,
                join_scope: runtime_join_scope,
            });
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
    let mut queue: VecDeque<usize> = graph.roots.iter().copied().collect();
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
