//! Lightweight TaskGroup for cooperative parallel task spawning.
//!
//! This module provides a TaskGroup API similar to Taskflow's design:
//! - Shared runtime-child fast path with no RunHandle allocation per child
//! - Simplified API without RuntimeCtx parameter
//! - Cooperative waiting with work-stealing

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use crate::error::FlowError;
use crate::executor::Executor;
use crate::runtime::RuntimeJoinScope;
use crate::util::WaitBackoff;

pub(crate) struct TaskGroupState {
    join_scope: RuntimeJoinScope,
    cancelled: AtomicBool,
}

impl TaskGroupState {
    fn new(cancelled: bool) -> Self {
        Self {
            join_scope: RuntimeJoinScope::default(),
            cancelled: AtomicBool::new(cancelled),
        }
    }

    pub(crate) fn add_child(&self) -> bool {
        self.join_scope.add_child()
    }

    pub(crate) fn finish_child(&self, result: Result<(), FlowError>) -> bool {
        self.join_scope.finish_child(result)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn is_idle(&self) -> bool {
        self.join_scope.is_idle()
    }

    fn take_error(&self) -> Option<FlowError> {
        self.join_scope.take_error()
    }

    fn pending_children(&self) -> usize {
        self.join_scope.pending_children()
    }
}

/// A lightweight task group for cooperative parallel task spawning.
///
/// Unlike `Executor::silent_async`, TaskGroup routes through the same
/// parent-linked child path used by runtime silent children, so child tasks do
/// not allocate a `RunHandle`.
///
/// # Example
///
/// ```ignore
/// executor.silent_async(move |runtime| {
///     let tg = runtime.task_group();
///     tg.silent_async(|| { /* task 1 */ });
///     tg.silent_async(|| { /* task 2 */ });
///     tg.corun(); // Wait for both tasks
/// });
/// ```
pub struct TaskGroup {
    executor: Executor,
    worker_id: Option<usize>,
    state: OnceLock<Arc<TaskGroupState>>,
    cancelled: AtomicBool,
}

impl TaskGroup {
    /// Create a new TaskGroup bound to a specific executor and worker.
    pub(crate) fn new(executor: Executor, worker_id: Option<usize>) -> Self {
        Self {
            executor,
            worker_id,
            state: OnceLock::new(),
            cancelled: AtomicBool::new(false),
        }
    }

    fn ensure_state(&self) -> &Arc<TaskGroupState> {
        self.state
            .get_or_init(|| Arc::new(TaskGroupState::new(self.cancelled.load(Ordering::Acquire))))
    }

    /// Spawn a task without waiting for its result.
    ///
    /// This is lighter weight than `RuntimeCtx::silent_async` because
    /// the closure doesn't need a RuntimeCtx parameter.
    pub fn silent_async<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.cancelled.load(Ordering::Acquire) {
            return;
        }

        let state = Arc::clone(self.ensure_state());
        if self.cancelled.load(Ordering::Acquire) {
            state.cancel();
            return;
        }

        self.executor
            .schedule_task_group_child(Box::new(task), self.worker_id, state);
    }

    /// Wait for all spawned tasks to complete.
    ///
    /// This uses cooperative waiting - the worker participates in
    /// work-stealing while waiting for the counter to reach zero.
    pub fn corun(&self) -> Result<(), FlowError> {
        let Some(state) = self.state.get() else {
            return Ok(());
        };

        if state.is_idle() {
            if state.take_error().is_some() {
                return Err(FlowError::plain("task in task group panicked"));
            }
            return Ok(());
        }

        match self.worker_id {
            Some(worker_id) => self
                .executor
                .wait_until_inline(worker_id, || state.is_idle()),
            None => {
                let mut backoff = WaitBackoff::new();
                while !state.is_idle() {
                    if !backoff.try_snooze() {
                        std::thread::yield_now();
                    }
                }
            }
        }

        if state.take_error().is_some() {
            return Err(FlowError::plain("task in task group panicked"));
        }

        Ok(())
    }

    /// Cancel all pending tasks in this group.
    ///
    /// Tasks that haven't started yet will skip execution.
    /// Tasks already running will continue to completion.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(state) = self.state.get() {
            state.cancel();
        }
    }

    /// Check if this task group has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Get the number of pending tasks.
    pub fn size(&self) -> usize {
        self.state.get().map_or(0, |state| state.pending_children())
    }
}
