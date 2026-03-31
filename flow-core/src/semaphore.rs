use std::sync::{Arc, Mutex};

use crate::executor::ScheduledTask;

#[derive(Clone, Default)]
pub struct Semaphore {
    inner: Arc<Mutex<SemaphoreState>>,
}

#[derive(Default)]
struct SemaphoreState {
    max_value: usize,
    current_value: usize,
    waiters: Vec<ScheduledTask>,
}

impl Semaphore {
    pub fn new(max_value: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SemaphoreState {
                max_value,
                current_value: max_value,
                waiters: Vec::new(),
            })),
        }
    }

    pub fn value(&self) -> usize {
        self.inner.lock().expect("semaphore poisoned").current_value
    }

    pub fn max_value(&self) -> usize {
        self.inner.lock().expect("semaphore poisoned").max_value
    }

    pub fn reset(&self) {
        let mut state = self.inner.lock().expect("semaphore poisoned");
        state.current_value = state.max_value;
        state.waiters.clear();
    }

    pub fn reset_with(&self, max_value: usize) {
        let mut state = self.inner.lock().expect("semaphore poisoned");
        state.max_value = max_value;
        state.current_value = max_value;
        state.waiters.clear();
    }

    pub(crate) fn try_acquire_or_wait(&self, task: ScheduledTask) -> bool {
        let mut state = self.inner.lock().expect("semaphore poisoned");
        if state.current_value > 0 {
            state.current_value -= 1;
            true
        } else {
            state.waiters.push(task);
            false
        }
    }

    pub(crate) fn release(&self, waiters: &mut Vec<ScheduledTask>) {
        let mut state = self.inner.lock().expect("semaphore poisoned");
        assert!(
            state.current_value < state.max_value,
            "cannot release a semaphore beyond its maximum value"
        );
        state.current_value += 1;
        waiters.append(&mut state.waiters);
    }
}
