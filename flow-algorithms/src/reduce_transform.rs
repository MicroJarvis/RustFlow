use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flow_core::{Executor, Flow, FlowError};

use crate::parallel_for::{ParallelForOptions, chunk_ranges_for_workers};

struct CompletionGuard {
    remaining: Arc<AtomicUsize>,
}

impl CompletionGuard {
    fn new(remaining: Arc<AtomicUsize>) -> Self {
        Self { remaining }
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        self.remaining.fetch_sub(1, Ordering::AcqRel);
    }
}

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

    let flow = Flow::new();
    let transform = Arc::new(transform);
    let chunks = chunk_ranges_for_workers(0..input.len(), options, executor.num_workers());
    let remaining = Arc::new(AtomicUsize::new(chunks.len()));
    let output = Arc::new(
        (0..input.len())
            .map(|_| Mutex::new(None))
            .collect::<Vec<_>>(),
    );

    for chunk in chunks {
        let input = Arc::clone(&input);
        let output = Arc::clone(&output);
        let transform = Arc::clone(&transform);
        let remaining = Arc::clone(&remaining);
        let chunk_start = chunk.start;
        let chunk_end = chunk.end;
        flow.spawn(move || {
            let _guard = CompletionGuard::new(Arc::clone(&remaining));
            for index in chunk_start..chunk_end {
                let value = transform(&input[index]);
                let mut slot = output[index]
                    .lock()
                    .expect("parallel transform slot poisoned");
                if slot.replace(value).is_some() {
                    panic!("parallel transform output assigned more than once");
                }
            }
        });
    }

    let result = executor.run(&flow).wait();
    while remaining.load(Ordering::Acquire) > 0 {
        std::thread::yield_now();
    }
    result?;

    Ok(output
        .iter()
        .map(|slot| {
            slot.lock()
                .expect("parallel transform slot poisoned")
                .take()
                .expect("parallel transform output should be initialized")
        })
        .collect())
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

    let flow = Flow::new();
    let reduce = Arc::new(reduce);
    let chunks = chunk_ranges_for_workers(0..input.len(), options, executor.num_workers());
    let remaining = Arc::new(AtomicUsize::new(chunks.len()));
    let partials = Arc::new(
        (0..chunks.len())
            .map(|_| Mutex::new(None))
            .collect::<Vec<_>>(),
    );

    for (chunk_index, chunk) in chunks.into_iter().enumerate() {
        let input = Arc::clone(&input);
        let partials = Arc::clone(&partials);
        let reduce = Arc::clone(&reduce);
        let remaining = Arc::clone(&remaining);
        let chunk_start = chunk.start;
        let chunk_end = chunk.end;
        flow.spawn(move || {
            let _guard = CompletionGuard::new(Arc::clone(&remaining));
            let mut iter = chunk_start..chunk_end;
            let first = iter
                .next()
                .expect("parallel reduce chunk should never be empty");
            let mut acc = input[first].clone();
            for index in iter {
                acc = reduce(acc, input[index].clone());
            }
            let mut slot = partials[chunk_index]
                .lock()
                .expect("parallel reduce partial slot poisoned");
            if slot.replace(acc).is_some() {
                panic!("parallel reduce partial assigned more than once");
            }
        });
    }

    let result = executor.run(&flow).wait();
    while remaining.load(Ordering::Acquire) > 0 {
        std::thread::yield_now();
    }
    result?;

    Ok(partials.iter().fold(init, |acc, partial| {
        reduce(
            acc,
            partial
                .lock()
                .expect("parallel reduce partial slot poisoned")
                .take()
                .expect("parallel reduce partial should be initialized"),
        )
    }))
}
