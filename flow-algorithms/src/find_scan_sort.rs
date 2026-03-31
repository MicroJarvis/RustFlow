use std::cmp::Ordering;
use std::sync::Arc;

use flow_core::{AsyncHandle, Executor, FlowError};

use crate::parallel_for::{ParallelForOptions, chunk_ranges};

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
    let handles = chunk_ranges(0..input.len(), options)
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
    options: ParallelForOptions,
    op: F,
) -> Result<Vec<T>, FlowError>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T, T) -> T + Send + Sync + 'static,
{
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let chunks = chunk_ranges(0..input.len(), options);
    let op = Arc::new(op);
    let chunk_prefixes = build_chunk_prefixes(executor, Arc::clone(&input), &chunks, &op, None)?;

    let handles = chunks
        .into_iter()
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let input = Arc::clone(&input);
            let op = Arc::clone(&op);
            let chunk_prefix = chunk_prefixes[chunk_index].clone();
            executor.async_task(move || {
                let mut indices = chunk.start..chunk.end;
                let first = indices
                    .next()
                    .expect("parallel inclusive scan chunk should not be empty");
                let mut output = Vec::with_capacity(chunk.end - chunk.start);
                let mut acc = match chunk_prefix {
                    Some(prefix) => op(prefix, input[first].clone()),
                    None => input[first].clone(),
                };
                output.push(acc.clone());
                for index in indices {
                    acc = op(acc, input[index].clone());
                    output.push(acc.clone());
                }
                output
            })
        })
        .collect::<Vec<_>>();

    Ok(wait_all(handles)?.into_iter().flatten().collect())
}

pub fn parallel_exclusive_scan<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    options: ParallelForOptions,
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

    let chunks = chunk_ranges(0..input.len(), options);
    let op = Arc::new(op);
    let chunk_prefixes =
        build_chunk_prefixes(executor, Arc::clone(&input), &chunks, &op, Some(init))?;

    let handles = chunks
        .into_iter()
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let input = Arc::clone(&input);
            let op = Arc::clone(&op);
            let chunk_prefix = chunk_prefixes[chunk_index].clone();
            executor.async_task(move || {
                let mut acc = chunk_prefix
                    .expect("parallel exclusive scan chunk prefix should be initialized");
                let mut output = Vec::with_capacity(chunk.end - chunk.start);
                for index in chunk.start..chunk.end {
                    output.push(acc.clone());
                    acc = op(acc, input[index].clone());
                }
                output
            })
        })
        .collect::<Vec<_>>();

    Ok(wait_all(handles)?.into_iter().flatten().collect())
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

    let compare = Arc::new(compare);
    let chunk_size = options.resolve_chunk_size(input.len());
    let handles = split_chunks(input, chunk_size)
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

    while chunks.len() > 1 {
        let mut next_round = Vec::with_capacity(chunks.len().div_ceil(2));
        let mut pending = chunks.into_iter();

        while let Some(left) = pending.next() {
            if let Some(right) = pending.next() {
                let compare = Arc::clone(&compare);
                next_round
                    .push(executor.async_task(move || merge_sorted(left, right, compare.as_ref())));
            } else {
                next_round.push(executor.async_task(move || left));
            }
        }

        chunks = wait_all(next_round)?;
    }

    Ok(chunks
        .pop()
        .expect("parallel sort should produce one output"))
}

fn build_chunk_prefixes<T, F>(
    executor: &Executor,
    input: Arc<[T]>,
    chunks: &[std::ops::Range<usize>],
    op: &Arc<F>,
    init: Option<T>,
) -> Result<Vec<Option<T>>, FlowError>
where
    T: Send + Sync + Clone + 'static,
    F: Fn(T, T) -> T + Send + Sync + 'static,
{
    let handles = chunks
        .iter()
        .cloned()
        .map(|chunk| {
            let input = Arc::clone(&input);
            let op = Arc::clone(op);
            executor.async_task(move || reduce_chunk(input, chunk, op.as_ref()))
        })
        .collect::<Vec<_>>();

    let chunk_totals = wait_all(handles)?;
    let mut prefixes = Vec::with_capacity(chunk_totals.len());
    let mut carry = init;

    for chunk_total in chunk_totals {
        prefixes.push(carry.clone());
        carry = Some(match carry.take() {
            Some(prefix) => op(prefix, chunk_total),
            None => chunk_total,
        });
    }

    Ok(prefixes)
}

fn reduce_chunk<T, F>(input: Arc<[T]>, chunk: std::ops::Range<usize>, op: &F) -> T
where
    T: Clone,
    F: Fn(T, T) -> T,
{
    let mut indices = chunk.start..chunk.end;
    let first = indices
        .next()
        .expect("parallel scan chunk should never be empty");
    let mut acc = input[first].clone();
    for index in indices {
        acc = op(acc, input[index].clone());
    }
    acc
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
