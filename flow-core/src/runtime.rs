use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::async_handle::{AsyncHandle, RuntimeAsyncState};
use crate::error::FlowError;
use crate::executor::{Executor, RunHandle};
use crate::flow::{Flow, TaskHandle, TaskId};

type RuntimeScheduler = Arc<dyn Fn(TaskId) + Send + Sync + 'static>;

#[derive(Default)]
pub(crate) struct RuntimeJoinScope {
    pending_children: std::sync::atomic::AtomicUsize,
    first_error: Mutex<Option<FlowError>>,
}

impl RuntimeJoinScope {
    pub(crate) fn add_child(&self) {
        self.pending_children.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn finish_child(&self, result: Result<(), FlowError>) {
        if let Err(error) = result {
            let mut slot = self
                .first_error
                .lock()
                .expect("runtime join scope poisoned");
            if slot.is_none() {
                *slot = Some(error);
            }
        }

        let previous = self.pending_children.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "runtime join scope child count underflow");
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.pending_children.load(Ordering::Acquire) == 0
    }

    pub(crate) fn take_error(&self) -> Option<FlowError> {
        self.first_error
            .lock()
            .expect("runtime join scope poisoned")
            .take()
    }
}

#[derive(Clone)]
pub struct RuntimeCtx {
    executor: Executor,
    worker_id: usize,
    scheduler: Option<RuntimeScheduler>,
    cancelled: Arc<AtomicBool>,
    join_scope: Arc<RuntimeJoinScope>,
}

impl RuntimeCtx {
    pub(crate) fn new(
        executor: Executor,
        worker_id: usize,
        scheduler: Option<RuntimeScheduler>,
        cancelled: Arc<AtomicBool>,
        join_scope: Arc<RuntimeJoinScope>,
    ) -> Self {
        Self {
            executor,
            worker_id,
            scheduler,
            cancelled,
            join_scope,
        }
    }

    pub fn executor(&self) -> &Executor {
        &self.executor
    }

    pub fn worker_id(&self) -> usize {
        self.worker_id
    }

    pub fn schedule(&self, task: TaskHandle) {
        if self.is_cancelled() {
            return;
        }

        match &self.scheduler {
            Some(scheduler) => scheduler(task.id()),
            None => panic!("runtime scheduling is unavailable in this context"),
        }
    }

    pub fn corun(&self, flow: &Flow) -> Result<(), FlowError> {
        self.executor.corun_inline(flow, self.worker_id)
    }

    pub fn corun_handle(&self, handle: &RunHandle) -> Result<(), FlowError> {
        self.executor.wait_handle_inline(handle, self.worker_id)
    }

    pub fn corun_handles(&self, handles: &[RunHandle]) -> Result<(), FlowError> {
        self.executor.wait_handles_inline(handles, self.worker_id)
    }

    pub fn silent_async<F>(&self, task: F)
    where
        F: FnOnce(&RuntimeCtx) + Send + 'static,
    {
        self.executor.schedule_runtime_silent_child(
            Box::new(move |runtime| {
                task(runtime);
                Ok(())
            }),
            self.worker_id,
            Arc::clone(&self.cancelled),
            Arc::clone(&self.join_scope),
        );
    }

    pub fn corun_children(&self) -> Result<(), FlowError> {
        self.executor
            .wait_until_inline(self.worker_id, || self.join_scope.is_idle());

        self.join_scope.take_error().map_or(Ok(()), Err)
    }

    pub fn wait_async<T>(&self, handle: AsyncHandle<T>) -> Result<T, FlowError> {
        handle.wait_with_runtime(self)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn wait_runtime_async_state<T>(
        &self,
        state: &RuntimeAsyncState<T>,
    ) -> Result<(), FlowError> {
        self.executor
            .wait_until_inline(self.worker_id, || state.is_finished());
        Ok(())
    }

    pub(crate) fn cancelled_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}
