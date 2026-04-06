use std::cell::UnsafeCell;
use std::sync::Arc;

use flow_core::{Executor, FlowError};

use crate::chunk_buffer::ChunkBuffer;
use crate::parallel_for::ParallelForOptions;
use crate::partitioner::{DynamicPartitioner, PartitionState, Partitioner};

const TRANSFORM_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 8_192;
const REDUCE_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 8_192;

/// Parallel transform using TaskGroup for lightweight scheduling.
///
/// This implementation avoids the overhead of Flow graph construction
/// and WriteOnceBuffer's per-element atomics by using:
/// - TaskGroup for direct task spawning without RunHandle allocation
/// - ChunkBuffer with exclusive chunk access (no per-element atomics)
pub fn parallel_transform<T, U, F>(
    executor: &Executor,
    input: Arc<[T]>,
    _options: ParallelForOptions,
    transform: F,
) -> Result<Vec<U>, FlowError>
where
    T: Send + Sync + 'static,
    U: Send + 'static,
    F: Fn(&T) -> U + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let workers = executor.num_workers().max(1);
    let len = input.len();

    // Sequential cutoff for small inputs
    if len <= workers.saturating_mul(TRANSFORM_SEQUENTIAL_CUTOFF_PER_WORKER) {
        return Ok(input.iter().map(|value| transform(value)).collect());
    }

    let w = workers.min(len);

    // Calculate balanced chunks for each worker
    let base_chunk = len / w;
    let remainder = len % w;

    let root_executor = executor.clone();
    executor
        .async_task(move || {
            let output: Arc<ChunkBuffer<U>> = Arc::new(ChunkBuffer::new_uninit(len));
            let transform_fn = Arc::new(transform);

            let tg = root_executor.task_group();

            for worker_id in 0..w {
                let start = worker_id * base_chunk + worker_id.min(remainder);
                let end = start + base_chunk + if worker_id < remainder { 1 } else { 0 };

                let input = Arc::clone(&input);
                let output = Arc::clone(&output);
                let transform = Arc::clone(&transform_fn);

                tg.silent_async(move || {
                    for i in start..end {
                        let value = transform(&input[i]);
                        unsafe {
                            output.write(i, value);
                        }
                    }
                });
            }

            tg.corun()?;

            // Convert buffer to Vec - safe because all workers have completed
            let output = Arc::try_unwrap(output).unwrap_or_else(|_| {
                panic!("all output references should be dropped after task completion")
            });
            Ok(unsafe { output.into_inner_assume_init() })
        })
        .wait()
        .and_then(|inner| inner)
}

/// Parallel reduce using TaskGroup for lightweight scheduling.
///
/// This implementation avoids the overhead of Flow graph construction
/// by using TaskGroup for direct task spawning. Each worker accumulates
/// partial results into its own cache-line padded slot to avoid false sharing.
pub fn parallel_reduce<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    _options: ParallelForOptions,
    init: T,
    reduce: F,
) -> Result<T, FlowError>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T, T) -> T + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(init);
    }

    let workers = executor.num_workers().max(1);
    let len = input.len();

    // Sequential cutoff for small inputs
    if len <= workers.saturating_mul(REDUCE_SEQUENTIAL_CUTOFF_PER_WORKER) {
        let mut iter = input.iter().cloned();
        let first = iter
            .next()
            .expect("parallel reduce sequential cutoff should preserve non-empty input");
        let partial = iter.fold(first, |acc, value| reduce(acc, value));
        return Ok(reduce(init, partial));
    }

    let w = workers.min(len);

    // Cache-line padded partial results to avoid false sharing
    #[repr(align(64))]
    struct CacheLine<T>(UnsafeCell<Option<T>>);

    unsafe impl<T: Send> Send for CacheLine<T> {}
    unsafe impl<T: Send> Sync for CacheLine<T> {}

    // Calculate balanced chunks for each worker
    let base_chunk = len / w;
    let remainder = len % w;

    let root_executor = executor.clone();
    executor
        .async_task(move || {
            let partials: Arc<Vec<CacheLine<T>>> = Arc::new(
                (0..w)
                    .map(|_| CacheLine(UnsafeCell::new(None)))
                    .collect(),
            );

            let reduce_fn = Arc::new(reduce);

            let tg = root_executor.task_group();

            for worker_id in 0..w {
                let start = worker_id * base_chunk + worker_id.min(remainder);
                let end = start + base_chunk + if worker_id < remainder { 1 } else { 0 };

                let input = Arc::clone(&input);
                let partials = Arc::clone(&partials);
                let reduce_fn = Arc::clone(&reduce_fn);

                tg.silent_async(move || {
                    // Process this worker's chunk
                    let mut acc = input[start].clone();
                    for i in (start + 1)..end {
                        acc = reduce_fn(acc, input[i].clone());
                    }

                    // Store partial result in this worker's exclusive slot
                    unsafe {
                        *partials[worker_id].0.get() = Some(acc);
                    }
                });
            }

            tg.corun()?;

            // Combine all partials (single-threaded, no atomics needed)
            let partials = Arc::try_unwrap(partials).unwrap_or_else(|_| {
                panic!("all partials references should be dropped after task completion")
            });

            let mut result = init;
            for slot in partials {
                if let Some(partial) = slot.0.into_inner() {
                    result = reduce_fn(result, partial);
                }
            }
            Ok(result)
        })
        .wait()
        .and_then(|inner| inner)
}

/// Parallel reduce with dynamic partitioning for better load balancing.
///
/// Unlike `parallel_reduce` which uses static pre-divided chunks,
/// this version uses atomic fetch_add for chunk claiming, allowing
/// fast workers to naturally help slow workers.
///
/// This is particularly useful when the work per element varies
/// (e.g., is_prime for larger numbers takes longer).
pub fn parallel_reduce_dynamic<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    chunk_size: usize,
    init: T,
    reduce: F,
) -> Result<T, FlowError>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T, T) -> T + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(init);
    }

    let workers = executor.num_workers().max(1);
    let len = input.len();

    if len <= workers.saturating_mul(REDUCE_SEQUENTIAL_CUTOFF_PER_WORKER) {
        let mut iter = input.iter().cloned();
        let first = iter
            .next()
            .expect("parallel reduce sequential cutoff should preserve non-empty input");
        let partial = iter.fold(first, |acc, value| reduce(acc, value));
        return Ok(reduce(init, partial));
    }

    let w = workers;

    // Cache-line padded partial results to avoid false sharing
    #[repr(align(64))]
    struct CacheLine<T>(UnsafeCell<Option<T>>);

    unsafe impl<T: Send> Send for CacheLine<T> {}
    unsafe impl<T: Send> Sync for CacheLine<T> {}

    let root_executor = executor.clone();
    executor
        .async_task(move || {
            let partials: Arc<Vec<CacheLine<T>>> = Arc::new(
                (0..w)
                    .map(|_| CacheLine(UnsafeCell::new(None)))
                    .collect(),
            );

            let reduce_fn = Arc::new(reduce);
            let state = Arc::new(PartitionState::new(len));
            let partitioner = Arc::new(DynamicPartitioner::new(chunk_size));

            let tg = root_executor.task_group();

            for worker_id in 0..w {
                let input = Arc::clone(&input);
                let partials = Arc::clone(&partials);
                let reduce_fn = Arc::clone(&reduce_fn);
                let state = Arc::clone(&state);
                let partitioner = Arc::clone(&partitioner);

                tg.silent_async(move || {
                    // Keep claiming chunks until exhausted
                    while let Some(chunk) = partitioner.next_chunk(&state) {
                        let mut iter = chunk.start..chunk.end;
                        if let Some(first) = iter.next() {
                            let mut acc = input[first].clone();
                            for index in iter {
                                acc = reduce_fn(acc, input[index].clone());
                            }

                            // Accumulate into this worker's slot
                            unsafe {
                                let slot = &partials[worker_id];
                                match *slot.0.get() {
                                    Some(ref existing) => {
                                        *slot.0.get() = Some(reduce_fn(existing.clone(), acc));
                                    }
                                    None => {
                                        *slot.0.get() = Some(acc);
                                    }
                                }
                            }
                        }
                    }
                });
            }

            tg.corun()?;

            // Combine all partials (single-threaded, no atomics needed)
            let partials = Arc::try_unwrap(partials).unwrap_or_else(|_| {
                panic!("all partials references should be dropped after task completion")
            });

            let mut result = init;
            for slot in partials {
                if let Some(partial) = slot.0.into_inner() {
                    result = reduce_fn(result, partial);
                }
            }
            Ok(result)
        })
        .wait()
        .and_then(|inner| inner)
}