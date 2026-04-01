use std::sync::{Arc, Mutex};

use crate::error::FlowError;
use crate::executor::RunHandle;
use crate::runtime::RuntimeCtx;

pub struct AsyncHandle<T> {
    run_handle: RunHandle,
    state: Arc<AsyncState<T>>,
}

impl<T> AsyncHandle<T> {
    pub(crate) fn new(run_handle: RunHandle, state: Arc<AsyncState<T>>) -> Self {
        Self { run_handle, state }
    }

    pub fn wait(self) -> Result<T, FlowError> {
        self.run_handle.wait()?;
        self.state
            .take()
            .ok_or_else(|| FlowError::plain("async task completed without producing a value"))
    }

    pub fn is_finished(&self) -> bool {
        self.run_handle.is_finished()
    }

    pub(crate) fn wait_with_runtime(self, runtime: &RuntimeCtx) -> Result<T, FlowError> {
        runtime.corun_handle(&self.run_handle)?;
        self.state
            .take()
            .ok_or_else(|| FlowError::plain("async task completed without producing a value"))
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
