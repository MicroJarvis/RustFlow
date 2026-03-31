use std::sync::Arc;

use crate::{
    AsyncHandle, Executor as CoreExecutor, Flow as CoreFlow, Observer, RunHandle, RuntimeCtx,
    Semaphore, Subflow as CoreSubflow, SubflowTaskHandle as CoreSubflowTask,
    TaskHandle as CoreTask, TaskId,
};

#[derive(Clone)]
pub struct Flow {
    inner: CoreFlow,
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

impl Flow {
    pub fn new() -> Self {
        Self {
            inner: CoreFlow::new(),
        }
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            inner: CoreFlow::with_name(name),
        }
    }

    pub fn emplace<F>(&self, task: F) -> Task
    where
        F: Fn() + Send + Sync + 'static,
    {
        Task::from_core(self.inner.spawn(task))
    }

    pub fn emplace_runtime<F>(&self, task: F) -> Task
    where
        F: Fn(&RuntimeCtx) + Send + Sync + 'static,
    {
        Task::from_core(self.inner.spawn_runtime(task))
    }

    pub fn emplace_condition<F>(&self, task: F) -> Task
    where
        F: Fn() -> usize + Send + Sync + 'static,
    {
        Task::from_core(self.inner.spawn_condition(task))
    }

    pub fn emplace_multi_condition<F>(&self, task: F) -> Task
    where
        F: Fn() -> Vec<usize> + Send + Sync + 'static,
    {
        Task::from_core(self.inner.spawn_multi_condition(task))
    }

    pub fn emplace_subflow<F>(&self, task: F) -> Task
    where
        F: Fn(&Subflow) + Send + Sync + 'static,
    {
        Task::from_core(self.inner.spawn_subflow(move |subflow| {
            let compat_subflow = Subflow::from_core(subflow.clone());
            task(&compat_subflow);
        }))
    }

    pub fn composed_of(&self, flow: &Flow) -> Task {
        Task::from_core(self.inner.compose(&flow.inner))
    }

    pub fn placeholder(&self) -> Task {
        Task::from_core(self.inner.placeholder())
    }

    pub fn name(&self) -> String {
        self.inner.name()
    }

    pub fn set_name(&self, name: impl Into<String>) {
        self.inner.set_name(name);
    }

    pub fn num_tasks(&self) -> usize {
        self.inner.num_tasks()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn dump_dot(&self) -> String {
        self.inner.dump_dot()
    }

    pub fn as_core(&self) -> &CoreFlow {
        &self.inner
    }
}

#[derive(Clone)]
pub struct Task {
    inner: CoreTask,
}

impl Task {
    fn from_core(inner: CoreTask) -> Self {
        Self { inner }
    }

    pub fn id(&self) -> TaskId {
        self.inner.id()
    }

    pub fn name(&self, name: impl Into<String>) -> Self {
        Self::from_core(self.inner.name(name))
    }

    pub fn has_work(&self) -> bool {
        self.inner.has_work()
    }

    pub fn acquire(&self, semaphore: &Semaphore) -> Self {
        Self::from_core(self.inner.acquire(semaphore))
    }

    pub fn release(&self, semaphore: &Semaphore) -> Self {
        Self::from_core(self.inner.release(semaphore))
    }

    pub fn precede<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = Task>,
    {
        let tasks = tasks.into_iter().map(|task| task.inner).collect::<Vec<_>>();
        Self::from_core(self.inner.precede(tasks))
    }

    pub fn succeed<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = Task>,
    {
        let tasks = tasks.into_iter().map(|task| task.inner).collect::<Vec<_>>();
        Self::from_core(self.inner.succeed(tasks))
    }
}

#[derive(Clone)]
pub struct Subflow {
    inner: CoreSubflow,
}

impl Subflow {
    fn from_core(inner: CoreSubflow) -> Self {
        Self { inner }
    }

    pub fn emplace<F>(&self, task: F) -> SubflowTask
    where
        F: Fn() + Send + Sync + 'static,
    {
        SubflowTask::from_core(self.inner.spawn(task))
    }

    pub fn emplace_runtime<F>(&self, task: F) -> SubflowTask
    where
        F: Fn(&RuntimeCtx) + Send + Sync + 'static,
    {
        SubflowTask::from_core(self.inner.spawn_runtime(task))
    }

    pub fn emplace_condition<F>(&self, task: F) -> SubflowTask
    where
        F: Fn() -> usize + Send + Sync + 'static,
    {
        SubflowTask::from_core(self.inner.spawn_condition(task))
    }

    pub fn emplace_multi_condition<F>(&self, task: F) -> SubflowTask
    where
        F: Fn() -> Vec<usize> + Send + Sync + 'static,
    {
        SubflowTask::from_core(self.inner.spawn_multi_condition(task))
    }

    pub fn emplace_subflow<F>(&self, task: F) -> SubflowTask
    where
        F: Fn(&Subflow) + Send + Sync + 'static,
    {
        SubflowTask::from_core(self.inner.spawn_subflow(move |subflow| {
            let compat_subflow = Subflow::from_core(subflow.clone());
            task(&compat_subflow);
        }))
    }

    pub fn placeholder(&self) -> SubflowTask {
        SubflowTask::from_core(self.inner.placeholder())
    }
}

#[derive(Clone)]
pub struct SubflowTask {
    inner: CoreSubflowTask,
}

impl SubflowTask {
    fn from_core(inner: CoreSubflowTask) -> Self {
        Self { inner }
    }

    pub fn id(&self) -> TaskId {
        self.inner.id()
    }

    pub fn name(&self, name: impl Into<String>) -> Self {
        Self::from_core(self.inner.name(name))
    }

    pub fn has_work(&self) -> bool {
        self.inner.has_work()
    }

    pub fn acquire(&self, semaphore: &Semaphore) -> Self {
        Self::from_core(self.inner.acquire(semaphore))
    }

    pub fn release(&self, semaphore: &Semaphore) -> Self {
        Self::from_core(self.inner.release(semaphore))
    }

    pub fn precede<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = SubflowTask>,
    {
        let tasks = tasks.into_iter().map(|task| task.inner).collect::<Vec<_>>();
        Self::from_core(self.inner.precede(tasks))
    }

    pub fn succeed<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = SubflowTask>,
    {
        let tasks = tasks.into_iter().map(|task| task.inner).collect::<Vec<_>>();
        Self::from_core(self.inner.succeed(tasks))
    }
}

#[derive(Clone)]
pub struct Executor {
    inner: CoreExecutor,
}

impl Executor {
    pub fn new(worker_count: usize) -> Self {
        Self {
            inner: CoreExecutor::new(worker_count),
        }
    }

    pub fn num_workers(&self) -> usize {
        self.inner.num_workers()
    }

    pub fn run(&self, flow: &Flow) -> RunHandle {
        self.inner.run(flow.as_core())
    }

    pub fn run_n(&self, flow: &Flow, n: usize) -> RunHandle {
        self.inner.run_n(flow.as_core(), n)
    }

    pub fn run_until<P>(&self, flow: &Flow, predicate: P) -> RunHandle
    where
        P: FnMut() -> bool + Send + 'static,
    {
        self.inner.run_until(flow.as_core(), predicate)
    }

    pub fn wait_for_all(&self) {
        self.inner.wait_for_all();
    }

    pub fn add_observer(&self, observer: Arc<dyn Observer>) {
        self.inner.add_observer(observer);
    }

    pub fn async_task<F, T>(&self, task: F) -> AsyncHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.inner.async_task(task)
    }

    pub fn silent_async<F>(&self, task: F) -> RunHandle
    where
        F: FnOnce() + Send + 'static,
    {
        self.inner.silent_async(task)
    }

    pub fn as_core(&self) -> &CoreExecutor {
        &self.inner
    }
}
