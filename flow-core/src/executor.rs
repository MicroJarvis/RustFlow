use std::any::Any;
use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicUsize, Ordering, fence};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};

use crossbeam_deque::{Injector, Steal, Stealer, Worker};

use crate::async_handle::{AsyncHandle, AsyncState, RuntimeAsyncState};
use crate::error::FlowError;
use crate::flow::{Flow, GraphSnapshot, NodeSnapshot, Subflow, TaskId, TaskKind};
use crate::observer::Observer;
use crate::runtime::{RuntimeCtx, RuntimeJoinScope};
use crate::task_group::{TaskGroup, TaskGroupState};
use crate::util::WaitBackoff;

const WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone)]
struct RuntimeSpawnContext {
    executor: Arc<ExecutorInner>,
    worker_id: usize,
    cancelled: Arc<AtomicBool>,
    // SAFETY: This points to the active RuntimeCtx.join_scope OnceLock for the
    // duration of the installed runtime spawn context guard.
    join_scope: *const OnceLock<Arc<RuntimeJoinScope>>,
}

impl RuntimeSpawnContext {
    fn new(
        executor: Arc<ExecutorInner>,
        worker_id: usize,
        cancelled: Arc<AtomicBool>,
        join_scope: *const OnceLock<Arc<RuntimeJoinScope>>,
    ) -> Self {
        Self {
            executor,
            worker_id,
            cancelled,
            join_scope,
        }
    }

    fn ensure_join_scope(&self) -> Arc<RuntimeJoinScope> {
        // SAFETY: `join_scope` points to the active RuntimeCtx held alive by the
        // scope guard installed around runtime task execution.
        unsafe { Arc::clone((&*self.join_scope).get_or_init(RuntimeJoinScope::default_arc)) }
    }
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

        // Pre-create worker queues and stealers
        let (workers, stealers): (Vec<_>, Vec<_>) = (0..worker_count)
            .map(|_| {
                let worker = Worker::new_lifo();
                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let parkers = (0..worker_count)
            .map(|_| Arc::new(WorkerParker::default()))
            .collect();

        let inner = Arc::new(ExecutorInner {
            stealers: stealers.into(),
            injector: Injector::new(),
            workers: Mutex::new(Some(workers)),
            parkers,
            run_completion: RunCompletionEvent::default(),
            worker_count,
            active_runs: AtomicUsize::new(0),
            live_run_states: Mutex::new(Vec::new()),
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
        self.start_run(flow.snapshot(), None)
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
        let mut backoff = WaitBackoff::new();
        while self.inner.active_runs.load(Ordering::Acquire) > 0 {
            if self.inner.shutdown.load(Ordering::Acquire) {
                return;
            }
            if backoff.try_snooze() {
                continue;
            }
            self.inner.run_completion.wait(
                &mut known_epoch,
                &self.inner.active_runs,
                &self.inner.shutdown,
            );
            backoff = WaitBackoff::new();
        }
    }

    pub fn corun_until<P>(&self, mut predicate: P)
    where
        P: FnMut() -> bool,
    {
        if let Some(worker_id) = self.current_worker_id() {
            self.wait_until_inline(worker_id, || predicate());
            return;
        }

        let mut backoff = WaitBackoff::new();
        while !predicate() {
            if !backoff.try_snooze() {
                thread::yield_now();
            }
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

    pub fn task_group(&self) -> TaskGroup {
        TaskGroup::new(self.clone(), self.current_worker_id())
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
            context.as_ref().map(RuntimeSpawnContext::ensure_join_scope),
        )
    }

    pub(crate) fn corun_inline(&self, flow: &Flow, worker_id: usize) -> Result<(), FlowError> {
        let handle = self.start_run(flow.snapshot(), Some(worker_id));
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

        let run_ptr: *const RunState = &*run;

        // Store Arc to keep RunState alive for the duration of this run.
        // It will be removed in finish_task when outstanding reaches 0.
        self.inner
            .live_run_states
            .lock()
            .expect("live_run_states poisoned")
            .push(run);

        let scheduled_roots = root_indices
            .into_iter()
            .map(|task_index| ScheduledTask {
                run: run_ptr,
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
        worker_hint: Option<usize>,
        runtime_cancelled: Arc<AtomicBool>,
        join_scope: Arc<RuntimeJoinScope>,
    ) {
        let _ = join_scope.add_child();
        self.inner.increment_active_runs();
        self.inner.enqueue_async(
            ScheduledAsyncTask::runtime_child(runner, runtime_cancelled, join_scope),
            worker_hint,
        );
    }

    pub(crate) fn schedule_task_group_child(
        &self,
        runner: Box<dyn FnOnce() + Send + 'static>,
        worker_hint: Option<usize>,
        state: Arc<TaskGroupState>,
    ) {
        if state.add_child() {
            self.inner.increment_active_runs();
        }
        self.inner.enqueue_task_group(
            ScheduledTaskGroupChild {
                runner: Some(runner),
                state,
            },
            worker_hint,
        );
    }

    fn current_runtime_spawn_context(&self) -> Option<RuntimeSpawnContext> {
        CURRENT_RUNTIME_SPAWN_CONTEXT.with(|slot| {
            let context = slot.borrow().clone()?;
            Arc::ptr_eq(&context.executor, &self.inner).then_some(context)
        })
    }

    fn current_worker_id(&self) -> Option<usize> {
        WORKER_LOCAL.with(|slot| slot.borrow().as_ref().map(|local| local.worker_id))
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
        // Resolve the worker reference once to avoid repeated TLS lookups.
        // SAFETY: The Worker lives in WORKER_LOCAL (thread-local storage) for the
        // entire worker thread lifetime. drive_worker_until is only called from
        // worker_loop or wait_until_inline (which runs on a worker thread).
        // The TLS is only cleared when WorkerLocalGuard drops after worker_loop returns.
        let worker_ptr: Option<*const Worker<ScheduledWork>> = WORKER_LOCAL.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|local| &local.worker as *const _)
        });

        let find_task = |inner: &ExecutorInner| {
            if let Some(w) = worker_ptr {
                // SAFETY: See above — pointer is valid for the duration of this function.
                let worker = unsafe { &*w };
                inner.next_task_with_worker(worker_id, worker)
            } else {
                inner.next_task(worker_id)
            }
        };

        let mut continuation = None;
        let mut steal_attempts = 0usize;
        const MAX_STEAL_ATTEMPTS: usize = 64;

        loop {
            if done() {
                return;
            }

            let task = continuation.take().or_else(|| find_task(&self.inner));
            if let Some(task) = task {
                steal_attempts = 0;
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

                    let task = find_task(&self.inner);
                    if let Some(task) = task {
                        self.inner.cancel_park(worker_id, observed_epoch);
                        continuation = self.inner.execute(self, worker_id, task);
                        continue;
                    }

                    self.inner
                        .commit_park(worker_id, observed_epoch, &self.inner.shutdown);
                }
                DriveMode::NoPark => {
                    // Aggressive work-stealing: try multiple times before yielding
                    // This matches Taskflow's _corun_until behavior
                    steal_attempts += 1;
                    if steal_attempts < MAX_STEAL_ATTEMPTS {
                        // Spin and try to steal again
                        std::hint::spin_loop();
                    } else {
                        // Reset counter and yield to allow other threads to run
                        steal_attempts = 0;
                        thread::yield_now();
                    }
                }
            }
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) != 1 {
            return;
        }

        self.inner.shutdown.store(true, Ordering::Release);
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

    fn wait(&self, known_epoch: &mut u64, active_runs: &AtomicUsize, shutdown: &AtomicBool) {
        let mut epoch = self.epoch.lock().expect("run completion poisoned");
        while *epoch == *known_epoch
            && active_runs.load(Ordering::Acquire) > 0
            && !shutdown.load(Ordering::Acquire)
        {
            epoch = self
                .ready
                .wait(epoch)
                .expect("run completion wait poisoned");
        }
        *known_epoch = *epoch;
    }
}

// Per-worker local state stored in thread-local storage.
// This allows enqueue operations from within a worker to push directly to its local queue.
thread_local! {
    static WORKER_LOCAL: RefCell<Option<WorkerLocalState>> = RefCell::new(None);
}

struct WorkerLocalState {
    worker: Worker<ScheduledWork>,
    worker_id: usize,
}

struct WorkerLocalGuard;

impl Drop for WorkerLocalGuard {
    fn drop(&mut self) {
        WORKER_LOCAL.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

fn install_worker_local(worker: Worker<ScheduledWork>, worker_id: usize) -> WorkerLocalGuard {
    WORKER_LOCAL.with(|slot| {
        *slot.borrow_mut() = Some(WorkerLocalState { worker, worker_id });
    });
    WorkerLocalGuard
}

struct ExecutorInner {
    /// Stealers for each worker - used for work stealing between workers
    stealers: Arc<[Stealer<ScheduledWork>]>,
    /// Global injector for tasks submitted from external threads
    injector: Injector<ScheduledWork>,
    /// Workers - passed to worker threads during startup
    workers: Mutex<Option<Vec<Worker<ScheduledWork>>>>,
    parkers: Vec<Arc<WorkerParker>>,
    run_completion: RunCompletionEvent,
    worker_count: usize,
    active_runs: AtomicUsize,
    /// Keeps RunState allocations alive while tasks reference them via raw pointers.
    /// Only accessed at run start (push) and run completion (remove).
    live_run_states: Mutex<Vec<Arc<RunState>>>,
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
        let current_epoch = parker.epoch.load(Ordering::Relaxed);

        if parker.state.load(Ordering::Relaxed) != WORKER_PREPARING
            || current_epoch != observed_epoch
            || shutdown.load(Ordering::Acquire)
        {
            parker.state.store(WORKER_RUNNING, Ordering::Relaxed);
            if current_epoch != observed_epoch && !shutdown.load(Ordering::Acquire) {
                self.notify_worker_wake(worker_id);
            }
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
        let state = parker.state.load(Ordering::SeqCst);
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
        fence(Ordering::Release);
        for worker_id in 0..self.parkers.len() {
            let _ = self.wake_worker(worker_id);
        }
    }

    fn schedule(&self, task: ScheduledTask, worker_hint: Option<usize>) {
        task.run_state().outstanding.fetch_add(1, Ordering::AcqRel);
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
        task.run_state().outstanding.fetch_add(1, Ordering::AcqRel);
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

    fn enqueue_task_group(&self, task: ScheduledTaskGroupChild, worker_hint: Option<usize>) {
        self.enqueue(ScheduledWork::TaskGroup(task), worker_hint);
    }

    fn enqueue(&self, task: ScheduledWork, worker_hint: Option<usize>) {
        // Use a cell to track whether we pushed locally
        let mut task_opt = Some(task);

        WORKER_LOCAL.with(|slot| {
            let borrowed = slot.borrow();
            if let Some(local) = borrowed.as_ref() {
                // We're in a worker thread - push to local queue
                // But respect worker_hint if it's different from our worker_id
                if worker_hint.is_none() || worker_hint == Some(local.worker_id) {
                    if let Some(t) = task_opt.take() {
                        local.worker.push(t);
                    }
                }
            }
        });

        // If not pushed locally, push to injector
        if let Some(task) = task_opt {
            self.injector.push(task);

            // Wake a worker to process the task
            // Note: wake_worker uses SeqCst load on parker state, which together
            // with prepare_park's SeqCst fence provides the Dekker-pattern
            // synchronization needed to prevent missed wakeups.
            if let Some(target) = worker_hint {
                let _ = self.wake_worker(target);
            } else {
                // Wake any parked worker
                for worker_id in 0..self.parkers.len() {
                    if self.wake_worker(worker_id) {
                        break;
                    }
                }
            }
        }
    }

    fn enqueue_many(&self, tasks: Vec<ScheduledWork>, worker_hint: Option<usize>) {
        if tasks.is_empty() {
            return;
        }

        let mut tasks_opt = Some(tasks);

        WORKER_LOCAL.with(|slot| {
            let borrowed = slot.borrow();
            if let Some(local) = borrowed.as_ref() {
                if worker_hint.is_none() || worker_hint == Some(local.worker_id) {
                    if let Some(t) = tasks_opt.take() {
                        for task in t {
                            local.worker.push(task);
                        }
                    }
                }
            }
        });

        // If not pushed locally, push to injector
        if let Some(tasks) = tasks_opt {
            for task in tasks {
                self.injector.push(task);
            }

            // Wake workers (SeqCst load in wake_worker provides Dekker synchronization)
            if let Some(target) = worker_hint {
                let _ = self.wake_worker(target);
            } else {
                for worker_id in 0..self.parkers.len() {
                    let _ = self.wake_worker(worker_id);
                }
            }
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

    /// Find a task using the work-stealing pattern:
    /// 1. Pop from local worker queue (LIFO for cache locality)
    /// 2. Steal from global injector (FIFO)
    /// 3. Steal from other workers' queues
    fn next_task(&self, worker_id: usize) -> Option<ScheduledWork> {
        // Access local worker from thread-local
        WORKER_LOCAL.with(|slot| {
            let borrowed = slot.borrow();
            let worker = borrowed.as_ref().map(|local| &local.worker);

            if let Some(w) = worker {
                self.next_task_with_worker(worker_id, w)
            } else {
                // No local worker, try injector directly
                loop {
                    match self.injector.steal() {
                        Steal::Success(task) => return Some(task),
                        Steal::Retry => continue,
                        Steal::Empty => break,
                    }
                }
                self.steal_from_others(worker_id)
            }
        })
    }

    /// Fast path: find a task with a pre-resolved worker reference, avoiding TLS lookup.
    fn next_task_with_worker(
        &self,
        worker_id: usize,
        worker: &Worker<ScheduledWork>,
    ) -> Option<ScheduledWork> {
        // 1. Try local queue first
        if let Some(task) = worker.pop() {
            return Some(task);
        }

        // 2. Try stealing from injector
        match self.injector.steal_batch_and_pop(worker) {
            Steal::Success(task) => return Some(task),
            Steal::Retry | Steal::Empty => {}
        }

        // 3. Try stealing from other workers
        self.steal_from_others(worker_id)
    }

    fn steal_from_others(&self, worker_id: usize) -> Option<ScheduledWork> {
        let stealers = &self.stealers;
        let num_stealers = stealers.len();
        if num_stealers == 0 {
            return None;
        }

        // Start from a rotating position to avoid all workers targeting the same victim
        let start = (worker_id + 1) % num_stealers;

        for offset in 0..num_stealers {
            let victim = (start + offset) % num_stealers;
            if victim == worker_id {
                continue;
            }
            match stealers[victim].steal() {
                Steal::Success(task) => return Some(task),
                Steal::Retry | Steal::Empty => continue,
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
            ScheduledWork::TaskGroup(task) => {
                self.execute_task_group_child(executor, worker_id, task);
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
        let run = scheduled.run_state();
        let node = &run.graph.nodes[scheduled.task_index];
        let mut continuation = None;

        if run.is_cancelled() {
            self.finish_task(&scheduled);
            return None;
        }

        if !self.acquire_all(node, &scheduled, Some(worker_id)) {
            return None;
        }

        // Transmute pointer to usize to cross the Send boundary in the scheduler closure.
        // SAFETY: Same invariant as ScheduledTask — RunState is valid for the run's lifetime.
        let run_addr = scheduled.run as usize;
        let executor_for_scheduler = executor.clone();
        let scheduler = Arc::new(move |task_id: TaskId| {
            executor_for_scheduler.inner.schedule(
                ScheduledTask {
                    run: run_addr as *const RunState,
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
            Arc::clone(&run.cancelled),
        );
        self.notify_task_finished(worker_id, node);

        if let Err(error) = &selected_successors {
            run.record_error(error.clone());
        }

        self.release_all(node, Some(worker_id));

        if run.is_cancelled() {
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
                                run: scheduled.run,
                                task_index: successor,
                            },
                            Some(worker_id),
                        );
                    }
                }
            }
        } else if selected_successors.is_ok() {
            for &successor in &node.successors {
                let previous = run.pending[successor].fetch_sub(1, Ordering::AcqRel);
                debug_assert!(previous > 0, "successor pending count underflow");
                if previous == 1 {
                    self.schedule_ready_graph(
                        &mut continuation,
                        ScheduledTask {
                            run: scheduled.run,
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
                let _ = join_scope.finish_child(Ok(()));
            }
            self.decrement_active_runs();
            return;
        }

        let runner = scheduled
            .runner
            .take()
            .expect("async runner scheduled more than once");
        let runtime = RuntimeCtx::new(
            executor.clone(),
            worker_id,
            None,
            Arc::clone(&scheduled.runtime_cancelled),
        );
        let _runtime_context = install_runtime_spawn_context(RuntimeSpawnContext::new(
            Arc::clone(&executor.inner),
            worker_id,
            Arc::clone(&scheduled.runtime_cancelled),
            runtime.join_scope_ptr(),
        ));
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
            let _ = join_scope.finish_child(completion_result);
        }
        self.decrement_active_runs();
    }

    fn execute_task_group_child(
        &self,
        _executor: &Executor,
        _worker_id: usize,
        mut scheduled: ScheduledTaskGroupChild,
    ) {
        if scheduled.state.is_cancelled() {
            if scheduled.state.finish_child(Ok(())) {
                self.decrement_active_runs();
            }
            return;
        }

        let runner = scheduled
            .runner
            .take()
            .expect("task-group runner scheduled more than once");
        let result = match panic::catch_unwind(AssertUnwindSafe(runner)) {
            Ok(()) => Ok(()),
            Err(payload) => Err(FlowError::plain(panic_payload_to_string(payload))),
        };

        if scheduled.state.finish_child(result) {
            self.decrement_active_runs();
        }
    }

    fn finish_task(&self, scheduled: &ScheduledTask) {
        let run = scheduled.run_state();
        let node = &run.graph.nodes[scheduled.task_index];
        run.pending[scheduled.task_index].store(node.predecessor_count, Ordering::Release);

        if run.outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
            let result = run.final_result();
            run.handle.complete(result);
            // Remove the Arc<RunState> from live_run_states, dropping it.
            // At this point no more ScheduledTasks reference this RunState.
            self.live_run_states
                .lock()
                .expect("live_run_states poisoned")
                .retain(|arc| !std::ptr::eq(&**arc, scheduled.run));
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

/// Lightweight task handle that avoids Arc ref-counting overhead per task.
/// The RunState is kept alive by `ExecutorInner::live_run_states` and only
/// removed when `outstanding` reaches 0 (all tasks for that run have completed).
#[derive(Clone, Copy)]
pub(crate) struct ScheduledTask {
    run: *const RunState,
    task_index: usize,
}

// SAFETY: RunState is Send+Sync. The raw pointer is valid for the lifetime of the run,
// which is guaranteed by the Arc<RunState> held in ExecutorInner::live_run_states.
unsafe impl Send for ScheduledTask {}
unsafe impl Sync for ScheduledTask {}

impl ScheduledTask {
    /// Access the RunState behind this task's pointer.
    ///
    /// # Safety contract
    /// The caller must ensure this task belongs to a run that is still alive
    /// (i.e., `outstanding > 0` and the Arc is still in `live_run_states`).
    /// This is upheld by construction: tasks are only created for active runs,
    /// and the Arc is only removed when outstanding reaches 0 in `finish_task`.
    fn run_state(&self) -> &RunState {
        // SAFETY: see struct-level and method-level safety docs
        unsafe { &*self.run }
    }
}

enum ScheduledWork {
    Graph(ScheduledTask),
    Async(ScheduledAsyncTask),
    TaskGroup(ScheduledTaskGroupChild),
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

struct ScheduledTaskGroupChild {
    runner: Option<Box<dyn FnOnce() + Send + 'static>>,
    state: Arc<TaskGroupState>,
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
            .initial_pending
            .iter()
            .copied()
            .map(AtomicUsize::new)
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
    // Get our pre-created worker from the shared state
    let worker = {
        let mut workers = inner.workers.lock().expect("workers poisoned");
        if let Some(ref mut w) = *workers {
            if worker_id < w.len() {
                w.swap_remove(worker_id)
            } else {
                Worker::new_lifo()
            }
        } else {
            Worker::new_lifo()
        }
    };

    // Install worker in thread-local for enqueue operations
    let _guard = install_worker_local(worker, worker_id);

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
            let runtime = RuntimeCtx::new(
                executor.clone(),
                worker_id,
                scheduler,
                Arc::clone(&cancelled),
            );
            let _runtime_context = install_runtime_spawn_context(RuntimeSpawnContext::new(
                Arc::clone(&executor.inner),
                worker_id,
                cancelled,
                runtime.join_scope_ptr(),
            ));
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
            execute_graph_inline(snapshot.as_ref(), executor, worker_id, cancelled)?;
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
    const SMALL_INLINE_GRAPH_LIMIT: usize = 32;

    if graph.nodes.is_empty() {
        return Ok(());
    }

    if graph.nodes.len() <= SMALL_INLINE_GRAPH_LIMIT
        && graph.nodes.iter().all(|node| !node.kind.is_condition())
    {
        return execute_graph_inline_small(graph, executor, worker_id, cancelled);
    }

    let mut pending = graph.initial_pending.clone();
    let mut queue = graph.roots.clone();
    let mut head = 0usize;
    let mut outstanding = queue.len();

    while head < queue.len() {
        let task_index = queue[head];
        head += 1;
        let node = &graph.nodes[task_index];
        let initial_pending = graph.initial_pending[task_index];
        if cancelled.load(Ordering::Acquire) {
            pending[task_index] = initial_pending;
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
            pending[task_index] = initial_pending;
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
                        queue.push(successor);
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
                    queue.push(successor);
                    outstanding += 1;
                }
            }
        }

        pending[task_index] = initial_pending;
        outstanding -= 1;
        if outstanding == 0 {
            break;
        }
    }

    Ok(())
}

fn execute_graph_inline_small(
    graph: &GraphSnapshot,
    executor: &Executor,
    worker_id: usize,
    cancelled: Arc<AtomicBool>,
) -> Result<(), FlowError> {
    const SMALL_INLINE_GRAPH_LIMIT: usize = 32;

    let mut pending = [0usize; SMALL_INLINE_GRAPH_LIMIT];
    let mut queue = [0usize; SMALL_INLINE_GRAPH_LIMIT];
    let node_count = graph.nodes.len();
    let root_count = graph.roots.len();

    pending[..node_count].copy_from_slice(&graph.initial_pending);
    queue[..root_count].copy_from_slice(&graph.roots);

    let mut head = 0usize;
    let mut tail = root_count;
    let mut outstanding = root_count;

    while head < tail {
        let task_index = queue[head];
        head += 1;

        let node = &graph.nodes[task_index];
        let initial_pending = graph.initial_pending[task_index];

        if cancelled.load(Ordering::Acquire) {
            pending[task_index] = initial_pending;
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

        debug_assert!(
            selected_successors.is_none(),
            "small inline DAG path should not execute condition nodes"
        );

        if cancelled.load(Ordering::Acquire) {
            pending[task_index] = initial_pending;
            outstanding -= 1;
            if outstanding == 0 {
                break;
            }
            continue;
        }

        for &successor in &node.successors {
            let previous = pending[successor];
            debug_assert!(previous > 0, "successor pending count underflow");
            pending[successor] = previous - 1;
            if previous == 1 {
                debug_assert!(
                    tail < SMALL_INLINE_GRAPH_LIMIT,
                    "small inline DAG queue overflowed unexpectedly"
                );
                queue[tail] = successor;
                tail += 1;
                outstanding += 1;
            }
        }

        pending[task_index] = initial_pending;
        outstanding -= 1;
        if outstanding == 0 {
            break;
        }
    }

    Ok(())
}
