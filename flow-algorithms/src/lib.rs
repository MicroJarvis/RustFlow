//! Algorithm adapters built on top of `flow-core`.
//!
//! The crate stays intentionally thin and reuses the `flow-core` scheduler,
//! but it is no longer just a placeholder: it currently exposes
//! `parallel_for`, `reduce`, `transform`, `find`, inclusive and exclusive
//! scan, and sort adapters.

mod find_scan_sort;
mod parallel_for;
mod reduce_transform;

pub use find_scan_sort::{
    parallel_exclusive_scan, parallel_find, parallel_inclusive_scan, parallel_sort,
    parallel_sort_by,
};
pub use flow_core::*;
pub use parallel_for::{ChunkSize, ParallelForExt, ParallelForOptions};
pub use reduce_transform::{parallel_reduce, parallel_transform};
