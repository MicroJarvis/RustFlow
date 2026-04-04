use std::ops::Range;
use std::sync::Arc;

use flow_core::{Flow, TaskHandle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkSize {
    Auto,
    Fixed(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParallelForOptions {
    chunk_size: ChunkSize,
}

impl Default for ParallelForOptions {
    fn default() -> Self {
        Self {
            chunk_size: ChunkSize::Auto,
        }
    }
}

impl ParallelForOptions {
    pub fn with_chunk_size(self, chunk_size: usize) -> Self {
        assert!(
            chunk_size > 0,
            "parallel_for chunk size must be greater than zero"
        );
        Self {
            chunk_size: ChunkSize::Fixed(chunk_size),
        }
    }

    pub(crate) fn resolve_chunk_size_for_workers(self, len: usize, workers: usize) -> usize {
        match self.chunk_size {
            ChunkSize::Fixed(chunk_size) => chunk_size,
            ChunkSize::Auto => {
                let target_chunks = workers.max(1).saturating_mul(4).max(1);
                len.div_ceil(target_chunks).max(1)
            }
        }
    }
}

pub(crate) fn chunk_ranges(range: Range<usize>, options: ParallelForOptions) -> Vec<Range<usize>> {
    let workers = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    chunk_ranges_for_workers(range, options, workers)
}

pub(crate) fn chunk_ranges_for_workers(
    range: Range<usize>,
    options: ParallelForOptions,
    workers: usize,
) -> Vec<Range<usize>> {
    let len = range.end.saturating_sub(range.start);
    if len == 0 {
        return Vec::new();
    }

    let chunk_size = options.resolve_chunk_size_for_workers(len, workers);
    let mut chunks = Vec::new();
    let mut chunk_start = range.start;

    while chunk_start < range.end {
        let chunk_end = chunk_start.saturating_add(chunk_size).min(range.end);
        chunks.push(chunk_start..chunk_end);
        chunk_start = chunk_end;
    }

    chunks
}

pub(crate) fn chunk_ranges_aligned_to_workers(
    range: Range<usize>,
    options: ParallelForOptions,
    workers: usize,
) -> Vec<Range<usize>> {
    match options.chunk_size {
        ChunkSize::Fixed(_) => chunk_ranges_for_workers(range, options, workers),
        ChunkSize::Auto => balanced_chunk_ranges_for_workers(range, workers),
    }
}

fn balanced_chunk_ranges_for_workers(range: Range<usize>, workers: usize) -> Vec<Range<usize>> {
    let len = range.end.saturating_sub(range.start);
    if len == 0 {
        return Vec::new();
    }

    let workers = workers.max(1).min(len);
    let base = len / workers;
    let remainder = len % workers;
    let mut chunks = Vec::with_capacity(workers);
    let mut chunk_start = range.start;

    for worker_index in 0..workers {
        let chunk_len = base + usize::from(worker_index < remainder);
        let chunk_end = chunk_start + chunk_len;
        chunks.push(chunk_start..chunk_end);
        chunk_start = chunk_end;
    }

    chunks
}

pub trait ParallelForExt {
    fn parallel_for<F>(
        &self,
        range: Range<usize>,
        options: ParallelForOptions,
        body: F,
    ) -> Vec<TaskHandle>
    where
        F: Fn(usize) + Send + Sync + 'static;
}

impl ParallelForExt for Flow {
    fn parallel_for<F>(
        &self,
        range: Range<usize>,
        options: ParallelForOptions,
        body: F,
    ) -> Vec<TaskHandle>
    where
        F: Fn(usize) + Send + Sync + 'static,
    {
        let body = Arc::new(body);
        chunk_ranges(range, options)
            .into_iter()
            .map(|chunk| {
                let body = Arc::clone(&body);
                let chunk_start = chunk.start;
                let chunk_end = chunk.end;
                self.spawn(move || {
                    for index in chunk_start..chunk_end {
                        body(index);
                    }
                })
            })
            .collect()
    }
}
