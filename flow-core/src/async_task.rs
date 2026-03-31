use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::async_handle::{AsyncHandle, AsyncState};
use crate::error::FlowError;
use crate::executor::{Executor, RunHandle, panic_payload_to_string};

#[derive(Clone, Default)]
pub struct AsyncTask {
    inner: Option<Arc<AsyncTaskState>>,
}

impl AsyncTask {
    fn new(inner: Arc<AsyncTaskState>) -> Self {
        Self { inner: Some(inner) }
    }
}

struct AsyncTaskState {
    executor: Executor,
    pending_dependencies: AtomicUsize,
    finished: AtomicBool,
    successors: Mutex<Vec<Arc<AsyncTaskState>>>,
    runner: Mutex<Option<Box<dyn FnOnce() -> Result<(), FlowError> + Send + 'static>>>,
    completion: RunHandle,
}

impl AsyncTaskState {
    fn new(
        executor: Executor,
        runner: Box<dyn FnOnce() -> Result<(), FlowError> + Send + 'static>,
    ) -> Self {
        Self {
            executor,
            pending_dependencies: AtomicUsize::new(1),
            finished: AtomicBool::new(false),
            successors: Mutex::new(Vec::new()),
            runner: Mutex::new(Some(runner)),
            completion: RunHandle::pending(),
        }
    }

    fn completion_handle(&self) -> RunHandle {
        self.completion.clone()
    }

    fn register_dependency(dependency: AsyncTask, successor: &Arc<Self>) {
        let Some(predecessor) = dependency.inner else {
            return;
        };

        successor
            .pending_dependencies
            .fetch_add(1, Ordering::AcqRel);

        let mut successors = predecessor
            .successors
            .lock()
            .expect("async task successors poisoned");

        if predecessor.finished.load(Ordering::Acquire) {
            drop(successors);
            successor.dependency_satisfied();
        } else {
            successors.push(Arc::clone(successor));
        }
    }

    fn dependency_satisfied(self: &Arc<Self>) {
        let previous = self.pending_dependencies.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "async dependency underflow");
        if previous == 1 {
            self.schedule();
        }
    }

    fn schedule(self: &Arc<Self>) {
        let state = Arc::clone(self);
        self.executor.silent_async(move || state.run());
    }

    fn run(self: Arc<Self>) {
        let runner = self
            .runner
            .lock()
            .expect("async task runner poisoned")
            .take()
            .expect("async task scheduled more than once");

        let result = match panic::catch_unwind(AssertUnwindSafe(runner)) {
            Ok(result) => result,
            Err(payload) => Err(FlowError::plain(panic_payload_to_string(payload))),
        };

        self.finish();
        self.completion.complete(result);
    }

    fn finish(&self) {
        let successors = {
            let mut successors = self
                .successors
                .lock()
                .expect("async task successors poisoned");
            self.finished.store(true, Ordering::Release);
            mem::take(&mut *successors)
        };

        for successor in successors {
            successor.dependency_satisfied();
        }
    }
}

impl Executor {
    pub fn silent_dependent_async<F, I>(&self, task: F, dependencies: I) -> AsyncTask
    where
        F: FnOnce() + Send + 'static,
        I: IntoIterator<Item = AsyncTask>,
    {
        let state = Arc::new(AsyncTaskState::new(
            self.clone(),
            Box::new(move || {
                task();
                Ok(())
            }),
        ));

        for dependency in dependencies {
            AsyncTaskState::register_dependency(dependency, &state);
        }

        if state.pending_dependencies.fetch_sub(1, Ordering::AcqRel) == 1 {
            state.schedule();
        }

        AsyncTask::new(state)
    }

    pub fn dependent_async<F, T, I>(&self, task: F, dependencies: I) -> (AsyncTask, AsyncHandle<T>)
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
        I: IntoIterator<Item = AsyncTask>,
    {
        let task_slot = Arc::new(Mutex::new(Some(task)));
        let value_state = Arc::new(AsyncState::new());

        let state = Arc::new(AsyncTaskState::new(
            self.clone(),
            Box::new({
                let task_slot = Arc::clone(&task_slot);
                let value_state = Arc::clone(&value_state);
                move || {
                    let task = task_slot
                        .lock()
                        .expect("dependent async task slot poisoned")
                        .take()
                        .expect("dependent async task executed more than once");
                    value_state.store(task());
                    Ok(())
                }
            }),
        ));

        for dependency in dependencies {
            AsyncTaskState::register_dependency(dependency, &state);
        }

        if state.pending_dependencies.fetch_sub(1, Ordering::AcqRel) == 1 {
            state.schedule();
        }

        let async_task = AsyncTask::new(Arc::clone(&state));
        let handle = AsyncHandle::new(state.completion_handle(), value_state);

        (async_task, handle)
    }
}
