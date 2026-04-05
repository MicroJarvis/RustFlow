use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::cell::UnsafeCell;

use flow_core::{AsyncHandle, Executor, Flow, FlowError};

use crate::parallel_for::{
    ParallelForOptions, chunk_ranges_for_workers,
};

const SCAN_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 32_768;

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
        *self.data[index].get() = value;
    }

    unsafe fn read(&self, index: usize) -> T
    where T: Clone
    {
        (*self.data[index].get()).clone()
    }

    fn into_inner(self) -> Vec<T> {
        self.data.into_iter().map(|cell| cell.into_inner()).collect()
    }
}

// SAFETY: Each worker only accesses its own exclusive chunk of the buffer.
// Workers never access the same indices, so there are no data races.
unsafe impl<T: Send> Send for ScanBuffer<T> {}
unsafe impl<T: Send> Sync for ScanBuffer<T> {}

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

    // Use a runtime task to coordinate
    let flow = Flow::new();
    let result = Arc::new(std::sync::Mutex::new(Ok(())));

    {
        let result = Arc::clone(&result);
        flow.spawn_runtime({
            let input = Arc::clone(&input);
            let output = Arc::clone(&output);
            let block_sums = Arc::clone(&block_sums);
            let counter = Arc::clone(&counter);
            let op = Arc::clone(&op);

            move |runtime| {
                for worker_id in 0..w {
                    let start = worker_id * base_chunk + worker_id.min(remainder);
                    let end = start + base_chunk + if worker_id < remainder { 1 } else { 0 };

                    if start >= end {
                        continue;
                    }

                    let input = Arc::clone(&input);
                    let output = Arc::clone(&output);
                    let block_sums = Arc::clone(&block_sums);
                    let counter = Arc::clone(&counter);
                    let op = Arc::clone(&op);

                    runtime.silent_async(move |_rt| {
                        // Phase 1: Local scan - write directly to output (no lock needed)
                        let mut acc = input[start].clone();
                        unsafe { output.write(start, acc.clone()); }

                        for i in start + 1..end {
                            acc = (op)(acc, input[i].clone());
                            unsafe { output.write(i, acc.clone()); }
                        }

                        // Store block sum
                        unsafe { *block_sums[worker_id].0.get() = acc; }

                        // Barrier - last worker computes global prefix
                        let arrived = counter.fetch_add(1, AtomicOrdering::AcqRel);
                        if arrived == w - 1 {
                            // Last worker: compute global prefix on block sums
                            let mut prefix: Option<T> = None;
                            for i in 0..w {
                                let bs = unsafe { (*block_sums[i].0.get()).clone() };
                                prefix = Some(match prefix {
                                    Some(p) => (op)(p, bs),
                                    None => bs,
                                });
                                unsafe { *block_sums[i].0.get() = prefix.clone().unwrap(); }
                            }
                            counter.store(0, AtomicOrdering::Release);
                        }

                        // Worker 0: no adjustment needed
                        if worker_id == 0 {
                            return;
                        }

                        // Wait for barrier using spin loop
                        while counter.load(AtomicOrdering::Acquire) != 0 {
                            std::hint::spin_loop();
                        }

                        // Phase 2: Apply prefix from previous block
                        let prefix = unsafe { (*block_sums[worker_id - 1].0.get()).clone() };
                        for i in start..end {
                            unsafe {
                                let current = output.read(i);
                                output.write(i, (op)(prefix.clone(), current));
                            }
                        }
                    });
                }

                if let Err(e) = runtime.corun_children() {
                    *result.lock().expect("result lock") = Err(e);
                }
            }
        });
    }

    executor.run(&flow).wait()?;
    result.lock().expect("result lock").clone()?;

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

    // Use a runtime task to coordinate
    let flow = Flow::new();
    let result = Arc::new(std::sync::Mutex::new(Ok(())));

    {
        let result = Arc::clone(&result);
        flow.spawn_runtime({
            let input = Arc::clone(&input);
            let output = Arc::clone(&output);
            let block_sums = Arc::clone(&block_sums);
            let counter = Arc::clone(&counter);
            let op = Arc::clone(&op);

            move |runtime| {
                for worker_id in 0..w {
                    let start = worker_id * base_chunk + worker_id.min(remainder);
                    let end = start + base_chunk + if worker_id < remainder { 1 } else { 0 };

                    if start >= end {
                        continue;
                    }

                    let input = Arc::clone(&input);
                    let output = Arc::clone(&output);
                    let block_sums = Arc::clone(&block_sums);
                    let counter = Arc::clone(&counter);
                    let op = Arc::clone(&op);
                    let init = init.clone();

                    runtime.silent_async(move |_rt| {
                        // Phase 1: Local exclusive scan
                        let mut acc = if worker_id == 0 { init.clone() } else { input[start].clone() };

                        for i in start..end {
                            unsafe { output.write(i, acc.clone()); }
                            acc = (op)(acc, input[i].clone());
                        }

                        // Store block sum
                        unsafe { *block_sums[worker_id].0.get() = acc; }

                        // Barrier - last worker computes global prefix
                        let arrived = counter.fetch_add(1, AtomicOrdering::AcqRel);
                        if arrived == w - 1 {
                            let mut prefix: Option<T> = None;
                            for i in 0..w {
                                let bs = unsafe { (*block_sums[i].0.get()).clone() };
                                prefix = Some(match prefix {
                                    Some(p) => (op)(p, bs),
                                    None => bs,
                                });
                                unsafe { *block_sums[i].0.get() = prefix.clone().unwrap(); }
                            }
                            counter.store(0, AtomicOrdering::Release);
                        }

                        if worker_id == 0 {
                            return;
                        }

                        // Wait for barrier
                        while counter.load(AtomicOrdering::Acquire) != 0 {
                            std::hint::spin_loop();
                        }

                        // Phase 2: Apply prefix
                        let prefix = unsafe { (*block_sums[worker_id - 1].0.get()).clone() };
                        for i in start..end {
                            unsafe {
                                let current = output.read(i);
                                output.write(i, (op)(prefix.clone(), current));
                            }
                        }
                    });
                }

                if let Err(e) = runtime.corun_children() {
                    *result.lock().expect("result lock") = Err(e);
                }
            }
        });
    }

    executor.run(&flow).wait()?;
    result.lock().expect("result lock").clone()?;

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

    // Larger cutoff to reduce parallel overhead
    let sequential_cutoff = executor.num_workers().max(1).saturating_mul(4096);
    if input.len() <= sequential_cutoff {
        let mut input = input;
        input.sort_unstable_by(|left, right| compare(left, right));
        return Ok(input);
    }

    let compare = Arc::new(compare);
    // Use larger chunk size to reduce merge rounds
    let chunk_size = options
        .resolve_chunk_size_for_workers(input.len(), executor.num_workers())
        .max(input.len() / executor.num_workers());
    let chunks = split_chunks(input, chunk_size);

    if chunks.len() <= 1 {
        return Ok(chunks
            .into_iter()
            .next()
            .expect("parallel sort should preserve the non-empty input chunk"));
    }

    let handles = chunks
        .into_iter()
        .map(|mut chunk| {
            let compare = Arc::clone(&compare);
            executor.async_task(move || {
                chunk.sort_unstable_by(|left, right| compare(left, right));
                chunk
            })
        })
        .collect::<Vec<_>>();

    let mut chunks = wait_all(handles)?;

    // Use sequential merge for small number of chunks to reduce scheduling overhead
    while chunks.len() > 1 {
        let parallel_merge = chunks.len() > 8;
        let mut next_chunks = Vec::with_capacity(chunks.len().div_ceil(2));
        let mut next_round = Vec::new();
        let mut pending = chunks.into_iter();

        while let Some(left) = pending.next() {
            if let Some(right) = pending.next() {
                if parallel_merge {
                    let compare = Arc::clone(&compare);
                    next_round.push(
                        executor.async_task(move || merge_sorted(left, right, compare.as_ref())),
                    );
                } else {
                    next_chunks.push(merge_sorted(left, right, compare.as_ref()));
                }
            } else {
                next_chunks.push(left);
            }
        }

        if parallel_merge {
            next_chunks.extend(wait_all(next_round)?);
        }
        chunks = next_chunks;
    }

    Ok(chunks
        .pop()
        .expect("parallel sort should produce one output"))
}

fn merge_sorted<T, F>(left: Vec<T>, right: Vec<T>, compare: &F) -> Vec<T>
where
    F: Fn(&T, &T) -> Ordering,
{
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut merged = Vec::with_capacity(left.len() + right.len());

    while let (Some(left_value), Some(right_value)) = (left.peek(), right.peek()) {
        if compare(left_value, right_value) != Ordering::Greater {
            merged.push(left.next().expect("left iterator should have a value"));
        } else {
            merged.push(right.next().expect("right iterator should have a value"));
        }
    }

    merged.extend(left);
    merged.extend(right);
    merged
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

fn split_chunks<T>(input: Vec<T>, chunk_size: usize) -> Vec<Vec<T>> {
    let mut chunks = Vec::new();
    let mut input = input.into_iter();

    loop {
        let mut chunk = Vec::with_capacity(chunk_size);
        for _ in 0..chunk_size {
            match input.next() {
                Some(value) => chunk.push(value),
                None => break,
            }
        }

        if chunk.is_empty() {
            break;
        }

        chunks.push(chunk);
    }

    chunks
}

fn wait_all<T>(handles: Vec<AsyncHandle<T>>) -> Result<Vec<T>, FlowError> {
    handles.into_iter().map(AsyncHandle::wait).collect()
}
