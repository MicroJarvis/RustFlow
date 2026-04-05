use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use flow_core::{AsyncHandle, Executor, FlowError};

use crate::parallel_for::{
    ParallelForOptions, chunk_ranges_for_workers,
};

const SCAN_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 2_048;
const SORT_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 2_048;

/// Thread-safe wrapper for UnsafeCell buffer where each worker has exclusive access to its chunk.
struct ScanBuffer<T> {
    data: Vec<UnsafeCell<T>>,
}

impl<T> ScanBuffer<T> {
    fn new(len: usize, default: T) -> Self
    where T: Clone
    {
        Self {
            data: (0..len).map(|_| UnsafeCell::new(default.clone())).collect(),
        }
    }

    unsafe fn write(&self, index: usize, value: T) {
        unsafe {
            *self.data[index].get() = value;
        }
    }

    unsafe fn read(&self, index: usize) -> T
    where T: Clone
    {
        unsafe { (*self.data[index].get()).clone() }
    }

    fn into_inner(self) -> Vec<T> {
        self.data.into_iter().map(|cell| cell.into_inner()).collect()
    }
}

// SAFETY: Each worker only accesses its own exclusive chunk of the buffer.
// Workers never access the same indices, so there are no data races.
unsafe impl<T: Send> Send for ScanBuffer<T> {}
unsafe impl<T: Send> Sync for ScanBuffer<T> {}

struct SharedMutPtr<T>(*mut T);

impl<T> Clone for SharedMutPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SharedMutPtr<T> {}

unsafe impl<T: Send> Send for SharedMutPtr<T> {}
unsafe impl<T: Send> Sync for SharedMutPtr<T> {}

impl<T> SharedMutPtr<T> {
    fn from_slice(values: &mut [T]) -> Self {
        Self(values.as_mut_ptr())
    }
}

pub fn parallel_find<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    options: ParallelForOptions,
    predicate: F,
) -> Result<Option<usize>, FlowError>
where
    T: Send + Sync + 'static,
    F: Fn(&T) -> bool + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(None);
    }

    let predicate = Arc::new(predicate);
    let handles = chunk_ranges_for_workers(0..input.len(), options, executor.num_workers())
        .into_iter()
        .map(|chunk| {
            let input = Arc::clone(&input);
            let predicate = Arc::clone(&predicate);
            executor.async_task(move || {
                (chunk.start..chunk.end).find(|index| predicate(&input[*index]))
            })
        })
        .collect::<Vec<_>>();

    Ok(wait_all(handles)?.into_iter().flatten().min())
}

pub fn parallel_inclusive_scan<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    _options: ParallelForOptions,
    op: F,
) -> Result<Vec<T>, FlowError>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T, T) -> T + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let workers = executor.num_workers().max(1);
    let n = input.len();

    if n <= workers.saturating_mul(SCAN_SEQUENTIAL_CUTOFF_PER_WORKER) {
        return Ok(sequential_inclusive_scan(&input, &op));
    }

    let w = workers.min(n);
    let op = Arc::new(op);
    let default = input[0].clone();

    // Pre-allocate output buffer - each worker has exclusive access to its chunk
    let output: Arc<ScanBuffer<T>> = Arc::new(ScanBuffer::new(n, default.clone()));

    // Block sums with cache-line padding
    #[repr(align(64))]
    struct CacheLine<T>(UnsafeCell<T>);
    unsafe impl<T: Send> Send for CacheLine<T> {}
    unsafe impl<T: Send> Sync for CacheLine<T> {}

    let block_sums: Arc<Vec<CacheLine<T>>> = Arc::new(
        (0..w).map(|_| CacheLine(UnsafeCell::new(default.clone()))).collect()
    );

    let counter = Arc::new(AtomicUsize::new(0));

    // Calculate balanced chunks
    let base_chunk = n / w;
    let remainder = n % w;

    let root_executor = executor.clone();
    let output_for_tasks = Arc::clone(&output);
    executor
        .async_task(move || -> Result<(), FlowError> {
            let tg = root_executor.task_group();

            for worker_id in 0..w {
                let start = worker_id * base_chunk + worker_id.min(remainder);
                let end = start + base_chunk + if worker_id < remainder { 1 } else { 0 };

                let task = {
                    let executor = root_executor.clone();
                    let input = Arc::clone(&input);
                    let output = Arc::clone(&output_for_tasks);
                    let block_sums = Arc::clone(&block_sums);
                    let counter = Arc::clone(&counter);
                    let op = Arc::clone(&op);

                    move || {
                        let mut acc = input[start].clone();
                        unsafe { output.write(start, acc.clone()) };

                        for i in start + 1..end {
                            acc = (op)(acc, input[i].clone());
                            unsafe { output.write(i, acc.clone()) };
                        }

                        unsafe { *block_sums[worker_id].0.get() = acc };

                        if counter.fetch_add(1, AtomicOrdering::AcqRel) == w - 1 {
                            let mut prefix: Option<T> = None;
                            for i in 0..w {
                                let block_sum = unsafe { (*block_sums[i].0.get()).clone() };
                                prefix = Some(match prefix {
                                    Some(running) => (op)(running, block_sum),
                                    None => block_sum,
                                });
                                unsafe {
                                    *block_sums[i].0.get() = prefix
                                        .clone()
                                        .expect("scan block prefix should exist");
                                }
                            }
                            counter.store(0, AtomicOrdering::Release);
                        }

                        if worker_id == 0 {
                            return;
                        }

                        executor.corun_until(|| counter.load(AtomicOrdering::Acquire) == 0);

                        let prefix = unsafe { (*block_sums[worker_id - 1].0.get()).clone() };
                        for i in start..end {
                            unsafe {
                                let current = output.read(i);
                                output.write(i, (op)(prefix.clone(), current));
                            }
                        }
                    }
                };

                if worker_id + 1 == w {
                    task();
                } else {
                    tg.silent_async(task);
                }
            }

            tg.corun()
        })
        .wait()??;

    // Collect results - safe because all workers have finished
    match Arc::try_unwrap(output) {
        Ok(buffer) => Ok(buffer.into_inner()),
        Err(_) => panic!("all references should be dropped"),
    }
}

pub fn parallel_exclusive_scan<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    _options: ParallelForOptions,
    init: T,
    op: F,
) -> Result<Vec<T>, FlowError>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T, T) -> T + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let workers = executor.num_workers().max(1);
    let n = input.len();

    if n <= workers.saturating_mul(SCAN_SEQUENTIAL_CUTOFF_PER_WORKER) {
        return Ok(sequential_exclusive_scan(&input, init, &op));
    }

    let w = workers.min(n);
    let op = Arc::new(op);
    let default = input[0].clone();

    // Pre-allocate output buffer
    let output: Arc<ScanBuffer<T>> = Arc::new(ScanBuffer::new(n, default.clone()));

    // Block sums with cache-line padding
    #[repr(align(64))]
    struct CacheLine<T>(UnsafeCell<T>);
    unsafe impl<T: Send> Send for CacheLine<T> {}
    unsafe impl<T: Send> Sync for CacheLine<T> {}

    let block_sums: Arc<Vec<CacheLine<T>>> = Arc::new(
        (0..w).map(|_| CacheLine(UnsafeCell::new(default.clone()))).collect()
    );

    let counter = Arc::new(AtomicUsize::new(0));

    // Calculate balanced chunks
    let base_chunk = n / w;
    let remainder = n % w;

    for worker_id in 0..w {
        let start = worker_id * base_chunk + worker_id.min(remainder);
        let initial = if worker_id == 0 {
            init.clone()
        } else {
            input[start - 1].clone()
        };
        unsafe {
            *block_sums[worker_id].0.get() = initial;
        }
    }

    let root_executor = executor.clone();
    let output_for_tasks = Arc::clone(&output);
    executor
        .async_task(move || -> Result<(), FlowError> {
            let tg = root_executor.task_group();

            for worker_id in 0..w {
                let start = worker_id * base_chunk + worker_id.min(remainder);
                let end = start + base_chunk + if worker_id < remainder { 1 } else { 0 };

                let task = {
                    let executor = root_executor.clone();
                    let input = Arc::clone(&input);
                    let output = Arc::clone(&output_for_tasks);
                    let block_sums = Arc::clone(&block_sums);
                    let counter = Arc::clone(&counter);
                    let op = Arc::clone(&op);

                    move || {
                        let mut acc = unsafe { (*block_sums[worker_id].0.get()).clone() };

                        for i in start + 1..end {
                            unsafe { output.write(i - 1, acc.clone()) };
                            acc = (op)(acc, input[i - 1].clone());
                        }
                        unsafe { output.write(end - 1, acc.clone()) };

                        unsafe { *block_sums[worker_id].0.get() = acc };

                        if counter.fetch_add(1, AtomicOrdering::AcqRel) == w - 1 {
                            let mut prefix: Option<T> = None;
                            for i in 0..w {
                                let block_sum = unsafe { (*block_sums[i].0.get()).clone() };
                                prefix = Some(match prefix {
                                    Some(running) => (op)(running, block_sum),
                                    None => block_sum,
                                });
                                unsafe {
                                    *block_sums[i].0.get() = prefix
                                        .clone()
                                        .expect("scan block prefix should exist");
                                }
                            }
                            counter.store(0, AtomicOrdering::Release);
                        }

                        if worker_id == 0 {
                            return;
                        }

                        executor.corun_until(|| counter.load(AtomicOrdering::Acquire) == 0);

                        let prefix = unsafe { (*block_sums[worker_id - 1].0.get()).clone() };
                        for i in start..end {
                            unsafe {
                                let current = output.read(i);
                                output.write(i, (op)(prefix.clone(), current));
                            }
                        }
                    }
                };

                if worker_id + 1 == w {
                    task();
                } else {
                    tg.silent_async(task);
                }
            }

            tg.corun()
        })
        .wait()??;

    match Arc::try_unwrap(output) {
        Ok(buffer) => Ok(buffer.into_inner()),
        Err(_) => panic!("all references should be dropped"),
    }
}

pub fn parallel_sort<T>(
    executor: &Executor,
    input: Vec<T>,
    options: ParallelForOptions,
) -> Result<Vec<T>, FlowError>
where
    T: Ord + Send + 'static,
{
    parallel_sort_by(executor, input, options, |left, right| left.cmp(right))
}

pub fn parallel_sort_by<T, F>(
    executor: &Executor,
    input: Vec<T>,
    options: ParallelForOptions,
    compare: F,
) -> Result<Vec<T>, FlowError>
where
    T: Send + 'static,
    F: Fn(&T, &T) -> Ordering + Send + Sync + 'static,
{
    if input.len() <= 1 {
        return Ok(input);
    }

    let sequential_cutoff = options
        .resolve_chunk_size_for_workers(input.len(), executor.num_workers())
        .max(executor.num_workers().max(1) * SORT_SEQUENTIAL_CUTOFF_PER_WORKER);

    if input.len() <= sequential_cutoff {
        let mut input = input;
        input.sort_unstable_by(|left, right| compare(left, right));
        return Ok(input);
    }

    let compare = Arc::new(compare);
    let len = input.len();
    let mut input = input;
    let data = SharedMutPtr::from_slice(input.as_mut_slice());
    let root_executor = executor.clone();

    executor
        .async_task(move || -> Result<(), FlowError> {
            unsafe {
                parallel_partition_sort(
                    root_executor,
                    data,
                    0,
                    len,
                    compare,
                    sequential_cutoff,
                )
            }
        })
        .wait()??;

    Ok(input)
}

fn sequential_inclusive_scan<T, F>(input: &[T], op: F) -> Vec<T>
where
    T: Clone,
    F: Fn(T, T) -> T,
{
    let mut iter = input.iter().cloned();
    let first = iter
        .next()
        .expect("sequential inclusive scan should preserve non-empty input");
    let mut output = Vec::with_capacity(input.len());
    let mut acc = first;
    output.push(acc.clone());

    for value in iter {
        acc = op(acc, value);
        output.push(acc.clone());
    }

    output
}

fn sequential_exclusive_scan<T, F>(input: &[T], init: T, op: F) -> Vec<T>
where
    T: Clone,
    F: Fn(T, T) -> T,
{
    let mut acc = init;
    let mut output = Vec::with_capacity(input.len());

    for value in input.iter().cloned() {
        output.push(acc.clone());
        acc = op(acc, value);
    }

    output
}

unsafe fn parallel_partition_sort<T, F>(
    executor: Executor,
    data: SharedMutPtr<T>,
    begin: usize,
    end: usize,
    compare: Arc<F>,
    sequential_cutoff: usize,
) -> Result<(), FlowError>
where
    T: Send + 'static,
    F: Fn(&T, &T) -> Ordering + Send + Sync + 'static,
{
    let len = end - begin;
    if len <= 1 {
        return Ok(());
    }

    if len <= sequential_cutoff {
        let slice = unsafe { std::slice::from_raw_parts_mut(data.0.add(begin), len) };
        slice.sort_unstable_by(|left, right| compare(left, right));
        return Ok(());
    }

    let mid = len / 2;
    let slice = unsafe { std::slice::from_raw_parts_mut(data.0.add(begin), len) };
    slice.select_nth_unstable_by(mid, |left, right| compare(left, right));

    let pivot = begin + mid;
    let tg = executor.task_group();
    let right_executor = executor.clone();
    let right_compare = Arc::clone(&compare);

    tg.silent_async(move || unsafe {
        parallel_partition_sort(
            right_executor,
            data,
            pivot + 1,
            end,
            right_compare,
            sequential_cutoff,
        )
        .expect("parallel sort right partition should succeed");
    });

    unsafe { parallel_partition_sort(executor, data, begin, pivot, compare, sequential_cutoff)? };
    tg.corun()
}

fn wait_all<T>(handles: Vec<AsyncHandle<T>>) -> Result<Vec<T>, FlowError> {
    handles.into_iter().map(AsyncHandle::wait).collect()
}
