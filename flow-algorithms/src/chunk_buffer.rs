//! Chunk-based buffer for parallel algorithms where each worker has exclusive
//! access to its assigned chunk, eliminating per-element atomic operations.

use std::cell::UnsafeCell;
use std::mem::{ManuallyDrop, MaybeUninit};

/// A buffer where each worker has exclusive access to its assigned chunk.
///
/// Unlike `WriteOnceBuffer`, this type does not use per-element atomics.
/// Safety is guaranteed by the caller ensuring each worker only accesses
/// its own chunk indices.
pub struct ChunkBuffer<T> {
    data: ManuallyDrop<Vec<UnsafeCell<MaybeUninit<T>>>>,
}

impl<T> ChunkBuffer<T> {
    /// Create a new uninitialized buffer of the given length.
    pub fn new_uninit(len: usize) -> Self {
        Self {
            data: ManuallyDrop::new(
                (0..len)
                    .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                    .collect(),
            ),
        }
    }

    /// Create a new buffer initialized with the given value.
    pub fn new(len: usize, value: T) -> Self
    where
        T: Clone,
    {
        Self {
            data: ManuallyDrop::new(
                (0..len)
                    .map(|_| UnsafeCell::new(MaybeUninit::new(value.clone())))
                    .collect(),
            ),
        }
    }

    /// Get the length of the buffer.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Write a value at the given index.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The index is within bounds
    /// - No other thread is accessing the same index concurrently
    /// - Each index is written exactly once before `into_inner_assume_init`
    #[inline]
    pub unsafe fn write(&self, index: usize, value: T) {
        unsafe {
            (*self.data[index].get()).write(value);
        }
    }

    /// Read a value at the given index.
    ///
    /// # Safety
    ///
    /// The caller must ensure the index has been initialized and no other
    /// thread is writing to the same index concurrently.
    #[inline]
    pub unsafe fn read(&self, index: usize) -> T
    where
        T: Clone,
    {
        unsafe { (*self.data[index].get()).assume_init_ref().clone() }
    }

    /// Get a pointer to the element at the given index.
    ///
    /// # Safety
    ///
    /// The caller must ensure the index is within bounds.
    #[inline]
    pub unsafe fn get_ptr(&self, index: usize) -> *mut T {
        unsafe { (*self.data[index].get()).as_mut_ptr() }
    }

    /// Convert the buffer into a `Vec<T>`, assuming all elements are initialized.
    ///
    /// # Safety
    ///
    /// The caller must ensure every element has been written exactly once.
    pub unsafe fn into_inner_assume_init(self) -> Vec<T> {
        let data = ManuallyDrop::into_inner(self.data);
        // SAFETY: Caller guarantees all elements are initialized
        unsafe {
            data.into_iter()
                .map(|cell| cell.into_inner().assume_init())
                .collect()
        }
    }

    /// Convert the buffer into a `Vec<MaybeUninit<T>>` without assuming initialization.
    pub fn into_inner(self) -> Vec<MaybeUninit<T>> {
        let data = ManuallyDrop::into_inner(self.data);
        data.into_iter().map(|cell| cell.into_inner()).collect()
    }
}

// SAFETY: The buffer is designed for chunk-based parallel access where each
// worker has exclusive access to its assigned chunk. As long as workers do
// not access each other's chunks, there are no data races.
unsafe impl<T: Send> Send for ChunkBuffer<T> {}
unsafe impl<T: Send> Sync for ChunkBuffer<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_uninit() {
        let buf: ChunkBuffer<u64> = ChunkBuffer::new_uninit(10);
        assert_eq!(buf.len(), 10);
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_new_initialized() {
        let buf: ChunkBuffer<u64> = ChunkBuffer::new(5, 42);
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn test_write_and_into_inner() {
        let buf: ChunkBuffer<u64> = ChunkBuffer::new_uninit(3);
        unsafe {
            buf.write(0, 1);
            buf.write(1, 2);
            buf.write(2, 3);
        }
        let vec = unsafe { buf.into_inner_assume_init() };
        assert_eq!(vec, vec![1, 2, 3]);
    }
}