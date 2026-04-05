//! Lightweight TaskGroup for cooperative parallel task spawning.
//!
//! This module provides a TaskGroup API similar to Taskflow's design:
//! - Shared runtime-child fast path with no RunHandle allocation per child
//! - Simplified API without RuntimeCtx parameter
//! - Cooperative waiting with work-stealing

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::FlowError;
use crate::executor::Executor;
use crate::runtime::RuntimeJoinScope;
use crate::util::WaitBackoff;

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
    join_scope: Arc<RuntimeJoinScope>,
    cancelled: Arc<AtomicBool>,
}

impl TaskGroup {
    /// Create a new TaskGroup bound to a specific executor and worker.
    pub(crate) fn new(executor: Executor, worker_id: Option<usize>) -> Self {
        Self {
            executor,
            worker_id,
            join_scope: Arc::new(RuntimeJoinScope::default()),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
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

        let cancelled = Arc::clone(&self.cancelled);
        self.executor.schedule_runtime_silent_child(
            Box::new(move |_| {
                if cancelled.load(Ordering::Acquire) {
                    return Ok(());
                }

                task();
                Ok(())
            }),
            self.worker_id,
            Arc::clone(&self.cancelled),
            Arc::clone(&self.join_scope),
        );
    }

    /// Wait for all spawned tasks to complete.
    ///
    /// This uses cooperative waiting - the worker participates in
    /// work-stealing while waiting for the counter to reach zero.
    pub fn corun(&self) -> Result<(), FlowError> {
        match self.worker_id {
            Some(worker_id) => self
                .executor
                .wait_until_inline(worker_id, || self.join_scope.is_idle()),
            None => {
                let mut backoff = WaitBackoff::new();
                while !self.join_scope.is_idle() {
                    if !backoff.try_snooze() {
                        std::thread::yield_now();
                    }
                }
            }
        }

        if self.join_scope.take_error().is_some() {
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
    }

    /// Check if this task group has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Get the number of pending tasks.
    pub fn size(&self) -> usize {
        self.join_scope.pending_children()
    }
}
