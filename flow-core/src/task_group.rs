//! Lightweight TaskGroup for cooperative parallel task spawning.
//!
//! This module provides a TaskGroup API similar to Taskflow's design:
//! - Embedded join counter (no Arc overhead for tracking)
//! - Simplified API without RuntimeCtx parameter
//! - Cooperative waiting with work-stealing

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::error::FlowError;
use crate::executor::Executor;

/// A lightweight task group for cooperative parallel task spawning.
///
/// Unlike `RuntimeCtx::silent_async`, TaskGroup uses an embedded join counter
/// instead of `Arc<RuntimeJoinScope>`, reducing per-task overhead.
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
    worker_id: usize,
    join_counter: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
    has_error: Arc<AtomicBool>,
}

impl TaskGroup {
    /// Create a new TaskGroup bound to a specific executor and worker.
    pub(crate) fn new(executor: Executor, worker_id: usize) -> Self {
        Self {
            executor,
            worker_id,
            join_counter: Arc::new(AtomicUsize::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            has_error: Arc::new(AtomicBool::new(false)),
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

        self.join_counter.fetch_add(1, Ordering::AcqRel);

        let counter = Arc::clone(&self.join_counter);
        let error_flag = Arc::clone(&self.has_error);
        let cancelled = Arc::clone(&self.cancelled);

        let _handle = self.executor.silent_async(move || {
            if cancelled.load(Ordering::Acquire) {
                counter.fetch_sub(1, Ordering::AcqRel);
                return;
            }

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task));

            counter.fetch_sub(1, Ordering::AcqRel);

            if result.is_err() {
                error_flag.store(true, Ordering::Release);
            }
        });
    }

    /// Wait for all spawned tasks to complete.
    ///
    /// This uses cooperative waiting - the worker participates in
    /// work-stealing while waiting for the counter to reach zero.
    pub fn corun(&self) -> Result<(), FlowError> {
        self.executor.wait_until_inline(self.worker_id, || {
            self.join_counter.load(Ordering::Acquire) == 0
        });

        if self.has_error.load(Ordering::Acquire) {
            Err(FlowError::plain("task in task group panicked"))
        } else {
            Ok(())
        }
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
        self.join_counter.load(Ordering::Acquire)
    }
}