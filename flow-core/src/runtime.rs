use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::FlowError;
use crate::executor::Executor;
use crate::flow::{Flow, TaskHandle, TaskId};

type RuntimeScheduler = Arc<dyn Fn(TaskId) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct RuntimeCtx {
    executor: Executor,
    worker_id: usize,
    scheduler: Option<RuntimeScheduler>,
    cancelled: Arc<AtomicBool>,
}

impl RuntimeCtx {
    pub(crate) fn new(
        executor: Executor,
        worker_id: usize,
        scheduler: Option<RuntimeScheduler>,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            executor,
            worker_id,
            scheduler,
            cancelled,
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

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}
