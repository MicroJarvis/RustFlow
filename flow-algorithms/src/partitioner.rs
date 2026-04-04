//! Partition strategies for parallel algorithms.
//!
//! This module provides different partitioning strategies for work distribution:
//! - `StaticPartitioner`: Pre-divided chunks (current default)
//! - `DynamicPartitioner`: Atomic fetch_add for work-stealing style
//! - `GuidedPartitioner`: Decreasing chunk sizes for balanced completion

use std::sync::atomic::{AtomicUsize, Ordering};
use std::ops::Range;

/// State shared across all workers for partitioning.
pub struct PartitionState {
    /// Next index to claim (for dynamic/guided partitioners)
    next: AtomicUsize,
    /// Total number of elements
    total: usize,
}

impl PartitionState {
    /// Create a new partition state for the given total count.
    pub fn new(total: usize) -> Self {
        Self {
            next: AtomicUsize::new(0),
            total,
        }
    }

    /// Get the total count.
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Trait for partitioning strategies.
///
/// Each partitioner decides how workers claim chunks of work.
pub trait Partitioner: Send + Sync {
    /// Claim the next chunk of work.
    ///
    /// Returns `None` when all work has been claimed.
    fn next_chunk(&self, state: &PartitionState) -> Option<Range<usize>>;
}

/// Static partitioner: pre-divided fixed chunks.
///
/// This is the simplest strategy but can lead to load imbalance
/// when work per element varies.
pub struct StaticPartitioner {
    chunks: Vec<Range<usize>>,
    next: AtomicUsize,
}

impl StaticPartitioner {
    /// Create a static partitioner with pre-computed chunks.
    pub fn new(chunks: Vec<Range<usize>>) -> Self {
        Self {
            chunks,
            next: AtomicUsize::new(0),
        }
    }
}

impl Partitioner for StaticPartitioner {
    fn next_chunk(&self, _state: &PartitionState) -> Option<Range<usize>> {
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        if index < self.chunks.len() {
            Some(self.chunks[index].clone())
        } else {
            None
        }
    }
}

/// Dynamic partitioner: atomic fetch_add for each chunk.
///
/// Workers compete to claim chunks via atomic counter.
/// This provides automatic load balancing - fast workers
/// naturally help slow workers by claiming more chunks.
///
/// Similar to Taskflow's `DynamicPartitioner`.
pub struct DynamicPartitioner {
    chunk_size: usize,
}

impl DynamicPartitioner {
    /// Create a dynamic partitioner with the given chunk size.
    pub fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }
}

impl Partitioner for DynamicPartitioner {
    fn next_chunk(&self, state: &PartitionState) -> Option<Range<usize>> {
        let start = state.next.fetch_add(self.chunk_size, Ordering::Relaxed);
        if start >= state.total {
            return None;
        }
        let end = (start + self.chunk_size).min(state.total);
        Some(start..end)
    }
}

/// Guided partitioner: decreasing chunk sizes.
///
/// Starts with large chunks and gradually decreases to small chunks.
/// This helps ensure all workers finish roughly at the same time,
/// as late-finishing workers get smaller chunks to process.
///
/// Similar to OpenMP's guided scheduling and Taskflow's `GuidedPartitioner`.
pub struct GuidedPartitioner {
    min_chunk_size: usize,
}

impl GuidedPartitioner {
    /// Create a guided partitioner with minimum chunk size.
    pub fn new(min_chunk_size: usize) -> Self {
        Self { min_chunk_size }
    }
}

impl Partitioner for GuidedPartitioner {
    fn next_chunk(&self, state: &PartitionState) -> Option<Range<usize>> {
        let remaining = state.total - state.next.load(Ordering::Relaxed);
        if remaining == 0 {
            return None;
        }

        // Guided formula: chunk_size = remaining / num_workers
        // But we don't know num_workers here, so use a simpler formula:
        // chunk_size = max(min_chunk, remaining / 2)
        // This gives exponentially decreasing chunks
        let chunk_size = (remaining / 2).max(self.min_chunk_size).min(remaining);

        let start = state.next.fetch_add(chunk_size, Ordering::Relaxed);
        if start >= state.total {
            return None;
        }
        let end = (start + chunk_size).min(state.total);
        Some(start..end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_partitioner() {
        let state = PartitionState::new(100);
        let partitioner = DynamicPartitioner::new(10);

        let mut claimed = Vec::new();
        while let Some(chunk) = partitioner.next_chunk(&state) {
            claimed.push(chunk);
        }

        assert_eq!(claimed.len(), 10);
        for (i, chunk) in claimed.iter().enumerate() {
            assert_eq!(*chunk, (i * 10)..((i + 1) * 10).min(100));
        }
    }

    #[test]
    fn test_dynamic_partitioner_partial() {
        let state = PartitionState::new(95);
        let partitioner = DynamicPartitioner::new(10);

        let mut claimed = Vec::new();
        while let Some(chunk) = partitioner.next_chunk(&state) {
            claimed.push(chunk);
        }

        assert_eq!(claimed.len(), 10);
        assert_eq!(claimed.last().unwrap().end, 95);
    }

    #[test]
    fn test_guided_partitioner() {
        let state = PartitionState::new(100);
        let partitioner = GuidedPartitioner::new(5);

        let mut claimed = Vec::new();
        while let Some(chunk) = partitioner.next_chunk(&state) {
            claimed.push(chunk);
        }

        // First chunk should be large (remaining/2)
        assert!(claimed[0].len() >= 25); // First chunk is ~50

        // Verify all elements covered
        let total: usize = claimed.iter().map(|c| c.len()).sum();
        assert_eq!(total, 100);
    }
}