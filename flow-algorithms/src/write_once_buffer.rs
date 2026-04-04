use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct WriteOnceBuffer<T> {
    slots: Vec<UnsafeCell<MaybeUninit<T>>>,
    initialized: Vec<AtomicBool>,
}

unsafe impl<T: Send> Send for WriteOnceBuffer<T> {}
unsafe impl<T: Send> Sync for WriteOnceBuffer<T> {}

impl<T> WriteOnceBuffer<T> {
    pub(crate) fn new(len: usize) -> Self {
        Self {
            slots: (0..len)
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
            initialized: (0..len).map(|_| AtomicBool::new(false)).collect(),
        }
    }

    pub(crate) fn write(&self, index: usize, value: T) {
        if self.initialized[index].swap(true, Ordering::AcqRel) {
            panic!("write-once buffer slot assigned more than once");
        }
        unsafe {
            (*self.slots[index].get()).write(value);
        }
    }

    pub(crate) fn take(&self, index: usize) -> T {
        if !self.initialized[index].swap(false, Ordering::AcqRel) {
            panic!("write-once buffer slot should be initialized");
        }
        unsafe { (*self.slots[index].get()).assume_init_read() }
    }
}

impl<T> Drop for WriteOnceBuffer<T> {
    fn drop(&mut self) {
        for index in 0..self.slots.len() {
            if self.initialized[index].load(Ordering::Acquire) {
                unsafe {
                    self.slots[index].get_mut().assume_init_drop();
                }
            }
        }
    }
}
