use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::error::FlowError;
use crate::executor::RunHandle;
use crate::runtime::RuntimeCtx;

pub struct AsyncHandle<T> {
    inner: AsyncHandleInner<T>,
}

enum AsyncHandleInner<T> {
    Standard {
        run_handle: RunHandle,
        state: Arc<AsyncState<T>>,
    },
    Runtime {
        state: Arc<RuntimeAsyncState<T>>,
    },
}

impl<T> AsyncHandle<T> {
    pub(crate) fn new(run_handle: RunHandle, state: Arc<AsyncState<T>>) -> Self {
        Self {
            inner: AsyncHandleInner::Standard { run_handle, state },
        }
    }

    pub(crate) fn from_runtime_state(state: Arc<RuntimeAsyncState<T>>) -> Self {
        Self {
            inner: AsyncHandleInner::Runtime { state },
        }
    }

    pub fn wait(self) -> Result<T, FlowError> {
        match self.inner {
            AsyncHandleInner::Standard { run_handle, state } => {
                run_handle.wait()?;
                state.take().ok_or_else(|| {
                    FlowError::plain("async task completed without producing a value")
                })
            }
            AsyncHandleInner::Runtime { state } => state.wait(),
        }
    }

    pub fn is_finished(&self) -> bool {
        match &self.inner {
            AsyncHandleInner::Standard { run_handle, .. } => run_handle.is_finished(),
            AsyncHandleInner::Runtime { state } => state.is_finished(),
        }
    }

    pub(crate) fn wait_with_runtime(self, runtime: &RuntimeCtx) -> Result<T, FlowError> {
        match self.inner {
            AsyncHandleInner::Standard { run_handle, state } => {
                runtime.corun_handle(&run_handle)?;
                state.take().ok_or_else(|| {
                    FlowError::plain("async task completed without producing a value")
                })
            }
            AsyncHandleInner::Runtime { state } => {
                runtime.wait_runtime_async_state(state.as_ref())?;
                state.wait()
            }
        }
    }
}

pub(crate) struct AsyncState<T> {
    value: Mutex<Option<T>>,
}

impl<T> AsyncState<T> {
    pub(crate) fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }

    pub(crate) fn store(&self, value: T) {
        *self.value.lock().expect("async state poisoned") = Some(value);
    }

    fn take(&self) -> Option<T> {
        self.value.lock().expect("async state poisoned").take()
    }
}

pub(crate) struct RuntimeAsyncState<T> {
    outcome: UnsafeCell<Option<Result<T, FlowError>>>,
    waiter: OnceLock<Box<RuntimeAsyncWaiter>>,
    finished: AtomicBool,
}

unsafe impl<T: Send> Send for RuntimeAsyncState<T> {}
unsafe impl<T: Send> Sync for RuntimeAsyncState<T> {}

#[derive(Default)]
struct RuntimeAsyncWaiter {
    lock: Mutex<()>,
    ready: Condvar,
}

impl<T> RuntimeAsyncState<T> {
    pub(crate) fn new() -> Self {
        Self {
            outcome: UnsafeCell::new(None),
            waiter: OnceLock::new(),
            finished: AtomicBool::new(false),
        }
    }

    pub(crate) fn complete_success(&self, value: T) {
        self.complete(Ok(value));
    }

    pub(crate) fn complete_error(&self, error: FlowError) {
        self.complete(Err(error));
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    fn wait(&self) -> Result<T, FlowError> {
        if self.finished.load(Ordering::Acquire) {
            return self.take_outcome();
        }

        let waiter = self
            .waiter
            .get_or_init(|| Box::new(RuntimeAsyncWaiter::default()));
        let mut guard = waiter.lock.lock().expect("runtime async wait poisoned");
        while !self.finished.load(Ordering::Acquire) {
            guard = waiter
                .ready
                .wait(guard)
                .expect("runtime async state wait poisoned");
        }
        drop(guard);
        self.take_outcome()
    }

    fn complete(&self, outcome: Result<T, FlowError>) {
        if let Some(waiter) = self.waiter.get() {
            let _guard = waiter.lock.lock().expect("runtime async wait poisoned");
            debug_assert!(
                !self.finished.load(Ordering::Relaxed),
                "runtime async state completed more than once"
            );
            unsafe {
                *self.outcome.get() = Some(outcome);
            }
            self.finished.store(true, Ordering::Release);
            waiter.ready.notify_all();
        } else {
            debug_assert!(
                !self.finished.load(Ordering::Relaxed),
                "runtime async state completed more than once"
            );
            unsafe {
                *self.outcome.get() = Some(outcome);
            }
            self.finished.store(true, Ordering::Release);
        }
    }

    fn take_outcome(&self) -> Result<T, FlowError> {
        unsafe {
            (*self.outcome.get())
                .take()
                .expect("runtime async outcome must be available after completion")
        }
    }
}
