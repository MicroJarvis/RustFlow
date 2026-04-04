use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flow_core::{Executor, Flow, FlowError};

use crate::parallel_for::{ParallelForOptions, chunk_ranges_aligned_to_workers};
use crate::partitioner::{DynamicPartitioner, Partitioner, PartitionState};
use crate::write_once_buffer::WriteOnceBuffer;

const TRANSFORM_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 8_192;
const REDUCE_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 8_192;

pub fn parallel_transform<T, U, F>(
    executor: &Executor,
    input: Arc<[T]>,
    options: ParallelForOptions,
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
    if input.len() <= workers.saturating_mul(TRANSFORM_SEQUENTIAL_CUTOFF_PER_WORKER) {
        return Ok(input.iter().map(|value| transform(value)).collect());
    }

    let flow = Flow::new();
    let transform = Arc::new(transform);
    let chunks = chunk_ranges_aligned_to_workers(0..input.len(), options, workers);
    let output = Arc::new(WriteOnceBuffer::new(input.len()));

    for chunk in chunks {
        let input = Arc::clone(&input);
        let output = Arc::clone(&output);
        let transform = Arc::clone(&transform);
        let chunk_start = chunk.start;
        let chunk_end = chunk.end;
        flow.spawn(move || {
            for index in chunk_start..chunk_end {
                let value = transform(&input[index]);
                output.write(index, value);
            }
        });
    }

    executor.run(&flow).wait()?;
    drop(flow);

    Ok((0..input.len()).map(|index| output.take(index)).collect())
}

pub fn parallel_reduce<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    options: ParallelForOptions,
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
    if input.len() <= workers.saturating_mul(REDUCE_SEQUENTIAL_CUTOFF_PER_WORKER) {
        let mut iter = input.iter().cloned();
        let first = iter
            .next()
            .expect("parallel reduce sequential cutoff should preserve non-empty input");
        let partial = iter.fold(first, |acc, value| reduce(acc, value));
        return Ok(reduce(init, partial));
    }

    let flow = Flow::new();
    let reduce = Arc::new(reduce);
    let chunks = chunk_ranges_aligned_to_workers(0..input.len(), options, workers);
    let num_chunks = chunks.len();
    let partials = Arc::new(WriteOnceBuffer::new(chunks.len()));

    for (chunk_index, chunk) in chunks.into_iter().enumerate() {
        let input = Arc::clone(&input);
        let partials = Arc::clone(&partials);
        let reduce = Arc::clone(&reduce);
        let chunk_start = chunk.start;
        let chunk_end = chunk.end;
        flow.spawn(move || {
            let mut iter = chunk_start..chunk_end;
            let first = iter
                .next()
                .expect("parallel reduce chunk should never be empty");
            let mut acc = input[first].clone();
            for index in iter {
                acc = reduce(acc, input[index].clone());
            }
            partials.write(chunk_index, acc);
        });
    }

    executor.run(&flow).wait()?;
    drop(flow);

    Ok((0..num_chunks).fold(init, |acc, chunk_index| {
        reduce(acc, partials.take(chunk_index))
    }))
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
    if input.len() <= workers.saturating_mul(REDUCE_SEQUENTIAL_CUTOFF_PER_WORKER) {
        let mut iter = input.iter().cloned();
        let first = iter
            .next()
            .expect("parallel reduce sequential cutoff should preserve non-empty input");
        let partial = iter.fold(first, |acc, value| reduce(acc, value));
        return Ok(reduce(init, partial));
    }

    let flow = Flow::new();
    let reduce = Arc::new(reduce);

    // Shared state for dynamic partitioning
    let state = Arc::new(PartitionState::new(input.len()));
    let partitioner = Arc::new(DynamicPartitioner::new(chunk_size));

    // Track how many partials have been written
    let partial_count = Arc::new(AtomicUsize::new(0));
    // Pre-allocate space for partials (worst case: input.len() / chunk_size + workers)
    let max_partials = (input.len() / chunk_size).max(1) + workers;
    let partials = Arc::new(WriteOnceBuffer::new(max_partials));

    // Spawn worker tasks
    for _ in 0..workers {
        let input = Arc::clone(&input);
        let partials = Arc::clone(&partials);
        let reduce = Arc::clone(&reduce);
        let state = Arc::clone(&state);
        let partitioner = Arc::clone(&partitioner);
        let partial_count = Arc::clone(&partial_count);

        flow.spawn(move || {
            // Keep claiming chunks until exhausted
            while let Some(chunk) = partitioner.next_chunk(&state) {
                let mut iter = chunk.start..chunk.end;
                if let Some(first) = iter.next() {
                    let mut acc = input[first].clone();
                    for index in iter {
                        acc = reduce(acc, input[index].clone());
                    }
                    // Write partial result
                    let partial_index = partial_count.fetch_add(1, Ordering::AcqRel);
                    partials.write(partial_index, acc);
                }
            }
        });
    }

    executor.run(&flow).wait()?;
    drop(flow);

    // Combine all partials
    let num_partials = partial_count.load(Ordering::Acquire);
    Ok((0..num_partials).fold(init, |acc, index| {
        reduce(acc, partials.take(index))
    }))
}
