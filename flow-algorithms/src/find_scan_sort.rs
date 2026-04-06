use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use flow_core::{AsyncHandle, Executor, FlowError, RuntimeCtx};

use crate::parallel_for::{ParallelForOptions, chunk_ranges_for_workers};

const SCAN_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 2_048;
// Sort tuning knobs. Keep these local until the benchmark contract is stable.
const SORT_SEQUENTIAL_CUTOFF_PER_WORKER: usize = 32_768;
const SORT_INSERTION_SORT_THRESHOLD: usize = 24;
const SORT_PARTIAL_INSERTION_SORT_LIMIT: usize = 8;
const SORT_NINTHER_THRESHOLD: usize = 128;
const SORT_IMBALANCED_PARTITION_DIVISOR: usize = 8;

/// Thread-safe wrapper for UnsafeCell buffer where each worker has exclusive access to its chunk.
struct ScanBuffer<T> {
    data: Vec<UnsafeCell<T>>,
}

impl<T> ScanBuffer<T> {
    fn new(len: usize, default: T) -> Self
    where
        T: Clone,
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
    where
        T: Clone,
    {
        unsafe { (*self.data[index].get()).clone() }
    }

    fn into_inner(self) -> Vec<T> {
        self.data
            .into_iter()
            .map(|cell| cell.into_inner())
            .collect()
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
        (0..w)
            .map(|_| CacheLine(UnsafeCell::new(default.clone())))
            .collect(),
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
                                    *block_sums[i].0.get() =
                                        prefix.clone().expect("scan block prefix should exist");
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
        (0..w)
            .map(|_| CacheLine(UnsafeCell::new(default.clone())))
            .collect(),
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
                                    *block_sums[i].0.get() =
                                        prefix.clone().expect("scan block prefix should exist");
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

    let workers = executor.num_workers().max(1);
    let sequential_cutoff = options
        .resolve_chunk_size_for_workers(input.len(), workers)
        .max(workers * SORT_SEQUENTIAL_CUTOFF_PER_WORKER);

    if workers <= 1 || input.len() <= sequential_cutoff {
        let mut input = input;
        input.sort_unstable_by(|left, right| compare(left, right));
        return Ok(input);
    }

    let is_less = Arc::new(move |left: &T, right: &T| compare(left, right) == Ordering::Less);
    let len = input.len();
    let bad_partition_budget = initial_sort_bad_partition_budget(len);
    let mut input = input;
    let data = SharedMutPtr::from_slice(input.as_mut_slice());

    executor
        .runtime_async(move |runtime| unsafe {
            parallel_sort_entry(
                runtime,
                data,
                len,
                is_less,
                sequential_cutoff,
                bad_partition_budget,
            )
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

fn ordering_from_is_less<T, F>(is_less: &F, left: &T, right: &T) -> Ordering
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    if is_less(left, right) {
        Ordering::Less
    } else if is_less(right, left) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

fn is_equal_by_less<T, F>(left: &T, right: &T, is_less: &F) -> bool
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    !is_less(left, right) && !is_less(right, left)
}

fn initial_sort_bad_partition_budget(len: usize) -> usize {
    len.checked_ilog2().unwrap_or(0) as usize * 2
}

fn insertion_sort_by_less<T, F>(slice: &mut [T], is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    for index in 1..slice.len() {
        let mut current = index;
        while current > 0 && is_less(&slice[current], &slice[current - 1]) {
            slice.swap(current, current - 1);
            current -= 1;
        }
    }
}

unsafe fn unguarded_insertion_sort_by_less<T, F>(
    data: SharedMutPtr<T>,
    begin: usize,
    end: usize,
    is_less: &F,
) where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    if end.saturating_sub(begin) < 2 {
        return;
    }

    for index in begin + 1..end {
        unsafe {
            if !is_less(&*data.0.add(index), &*data.0.add(index - 1)) {
                continue;
            }

            let mut current = index;
            let value = std::ptr::read(data.0.add(index));

            loop {
                std::ptr::copy(data.0.add(current - 1), data.0.add(current), 1);
                current -= 1;

                if !is_less(&value, &*data.0.add(current - 1)) {
                    break;
                }
            }

            std::ptr::write(data.0.add(current), value);
        }
    }
}

#[allow(dead_code)]
fn partial_insertion_sort_by_less<T, F>(slice: &mut [T], is_less: &F) -> bool
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    let mut moves = 0usize;

    for index in 1..slice.len() {
        if !is_less(&slice[index], &slice[index - 1]) {
            continue;
        }

        let mut current = index;
        while current > 0 && is_less(&slice[current], &slice[current - 1]) {
            slice.swap(current, current - 1);
            current -= 1;
            moves += 1;
            if moves > SORT_PARTIAL_INSERTION_SORT_LIMIT {
                return false;
            }
        }
    }

    true
}

fn sort2_by_less<T, F>(slice: &mut [T], a: usize, b: usize, is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    if is_less(&slice[b], &slice[a]) {
        slice.swap(a, b);
    }
}

fn sort3_by_less<T, F>(slice: &mut [T], a: usize, b: usize, c: usize, is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    sort2_by_less(slice, a, b, is_less);
    sort2_by_less(slice, b, c, is_less);
    sort2_by_less(slice, a, b, is_less);
}

fn choose_pivot_index_by_less<T, F>(slice: &mut [T], is_less: &F) -> usize
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    let len = slice.len();
    let mid = len / 2;

    if len < SORT_NINTHER_THRESHOLD {
        sort3_by_less(slice, mid, 0, len - 1, is_less);
        return 0;
    }

    sort3_by_less(slice, 0, mid, len - 1, is_less);
    sort3_by_less(slice, 1, mid - 1, len - 2, is_less);
    sort3_by_less(slice, 2, mid + 1, len - 3, is_less);
    sort3_by_less(slice, mid - 1, mid, mid + 1, is_less);
    slice.swap(0, mid);
    0
}

struct PartitionResult {
    left_len: usize,
    equal_len: usize,
    already_partitioned: bool,
}

fn partition_by_less<T, F>(slice: &mut [T], pivot_index: usize, is_less: &F) -> PartitionResult
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    let len = slice.len();
    debug_assert!(len > 0);

    let pivot_slot = len - 1;
    let mut already_partitioned = true;
    if pivot_index != pivot_slot {
        slice.swap(pivot_index, pivot_slot);
        already_partitioned = false;
    }

    let mut less = 0usize;
    let mut scan = 0usize;
    let mut greater = pivot_slot;

    while scan < greater {
        if is_less(&slice[scan], &slice[pivot_slot]) {
            if scan != less {
                slice.swap(scan, less);
                already_partitioned = false;
            }
            less += 1;
            scan += 1;
        } else if is_less(&slice[pivot_slot], &slice[scan]) {
            greater -= 1;
            if scan != greater {
                slice.swap(scan, greater);
                already_partitioned = false;
            }
        } else {
            scan += 1;
        }
    }

    if greater != pivot_slot {
        slice.swap(greater, pivot_slot);
        already_partitioned = false;
    }

    PartitionResult {
        left_len: less,
        equal_len: greater + 1 - less,
        already_partitioned,
    }
}

fn partition_equal_left_by_less<T, F>(slice: &mut [T], pivot_index: usize, is_less: &F) -> usize
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    debug_assert!(!slice.is_empty());

    if pivot_index != 0 {
        slice.swap(0, pivot_index);
    }

    let mut equal = 0usize;
    let mut scan = 1usize;
    let mut greater = slice.len();

    while scan < greater {
        if is_less(&slice[0], &slice[scan]) {
            greater -= 1;
            if scan != greater {
                slice.swap(scan, greater);
            }
        } else {
            equal += 1;
            if scan != equal {
                slice.swap(scan, equal);
            }
            scan += 1;
        }
    }

    if equal != 0 {
        slice.swap(0, equal);
    }

    equal + 1
}

fn break_patterns_by_less<T>(slice: &mut [T], left_len: usize, equal_len: usize) {
    let len = slice.len();
    let right_begin = left_len + equal_len;
    let right_len = len.saturating_sub(right_begin);

    if left_len >= SORT_INSERTION_SORT_THRESHOLD {
        slice.swap(0, left_len / 4);
        slice.swap(left_len - 1, left_len - left_len / 4);

        if left_len > SORT_NINTHER_THRESHOLD {
            slice.swap(1, left_len / 4 + 1);
            slice.swap(2, left_len / 4 + 2);
            slice.swap(left_len - 2, left_len - (left_len / 4 + 1));
            slice.swap(left_len - 3, left_len - (left_len / 4 + 2));
        }
    }

    if right_len >= SORT_INSERTION_SORT_THRESHOLD {
        slice.swap(right_begin, right_begin + right_len / 4);
        slice.swap(len - 1, len - right_len / 4);

        if right_len > SORT_NINTHER_THRESHOLD {
            slice.swap(right_begin + 1, right_begin + 1 + right_len / 4);
            slice.swap(right_begin + 2, right_begin + 2 + right_len / 4);
            slice.swap(len - 2, len - (1 + right_len / 4));
            slice.swap(len - 3, len - (2 + right_len / 4));
        }
    }
}

fn sift_down_by_less<T, F>(slice: &mut [T], mut root: usize, end: usize, is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    loop {
        let left_child = root.saturating_mul(2).saturating_add(1);
        if left_child >= end {
            return;
        }

        let mut child = left_child;
        let right_child = left_child + 1;
        if right_child < end && is_less(&slice[child], &slice[right_child]) {
            child = right_child;
        }

        if !is_less(&slice[root], &slice[child]) {
            return;
        }

        slice.swap(root, child);
        root = child;
    }
}

#[allow(dead_code)]
fn heapsort_by_less<T, F>(slice: &mut [T], is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    if slice.len() < 2 {
        return;
    }

    for start in (0..=(slice.len() - 2) / 2).rev() {
        sift_down_by_less(slice, start, slice.len(), is_less);
    }

    for end in (1..slice.len()).rev() {
        slice.swap(0, end);
        sift_down_by_less(slice, 0, end, is_less);
    }
}

fn sort_slice_by_less<T, F>(slice: &mut [T], is_less: &F)
where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    if slice.len() < SORT_INSERTION_SORT_THRESHOLD {
        insertion_sort_by_less(slice, is_less);
    } else {
        slice.sort_unstable_by(|left, right| ordering_from_is_less(is_less, left, right));
    }
}

unsafe fn sort_range_by_less<T, F>(
    data: SharedMutPtr<T>,
    begin: usize,
    end: usize,
    leftmost: bool,
    is_less: &F,
) where
    F: Fn(&T, &T) -> bool + ?Sized,
{
    let len = end.saturating_sub(begin);
    if len <= 1 {
        return;
    }

    if len < SORT_INSERTION_SORT_THRESHOLD && !leftmost {
        unsafe {
            unguarded_insertion_sort_by_less(data, begin, end, is_less);
        }
        return;
    }

    let slice = unsafe { std::slice::from_raw_parts_mut(data.0.add(begin), len) };
    sort_slice_by_less(slice, is_less);
}

unsafe fn parallel_sort_entry<T, F>(
    runtime: &RuntimeCtx,
    data: SharedMutPtr<T>,
    len: usize,
    is_less: Arc<F>,
    sequential_cutoff: usize,
    bad_partition_budget: usize,
) -> Result<(), FlowError>
where
    T: Send + 'static,
    F: Fn(&T, &T) -> bool + Send + Sync + 'static,
{
    unsafe {
        parallel_quicksort(
            runtime,
            data,
            0,
            len,
            is_less,
            sequential_cutoff,
            bad_partition_budget,
            true,
        )
    }
}

unsafe fn parallel_quicksort<T, F>(
    runtime: &RuntimeCtx,
    data: SharedMutPtr<T>,
    mut begin: usize,
    mut end: usize,
    is_less: Arc<F>,
    sequential_cutoff: usize,
    bad_partition_budget: usize,
    mut leftmost: bool,
) -> Result<(), FlowError>
where
    T: Send + 'static,
    F: Fn(&T, &T) -> bool + Send + Sync + 'static,
{
    let mut bad_partition_budget = bad_partition_budget;

    loop {
        let len = end - begin;
        if len <= 1 {
            break;
        }

        if len <= sequential_cutoff {
            unsafe {
                sort_range_by_less(data, begin, end, leftmost, is_less.as_ref());
            }
            break;
        }

        let slice = unsafe { std::slice::from_raw_parts_mut(data.0.add(begin), len) };
        if bad_partition_budget == 0 {
            heapsort_by_less(slice, is_less.as_ref());
            break;
        }

        let pivot_index = choose_pivot_index_by_less(slice, is_less.as_ref());

        if !leftmost
            && unsafe {
                is_equal_by_less(
                    &*data.0.add(begin - 1),
                    slice.get_unchecked(pivot_index),
                    is_less.as_ref(),
                )
            }
        {
            let equal_prefix_len =
                partition_equal_left_by_less(slice, pivot_index, is_less.as_ref());
            begin += equal_prefix_len;
            leftmost = false;
            continue;
        }

        let partition = partition_by_less(slice, pivot_index, is_less.as_ref());

        let left_begin = begin;
        let left_end = begin + partition.left_len;
        let right_begin = left_end + partition.equal_len;
        let right_end = end;
        let left_len = left_end - left_begin;
        let right_len = right_end - right_begin;
        let left_is_leftmost = leftmost;
        let right_is_leftmost = false;
        let highly_unbalanced = left_len < len / SORT_IMBALANCED_PARTITION_DIVISOR
            || right_len < len / SORT_IMBALANCED_PARTITION_DIVISOR;

        if highly_unbalanced {
            bad_partition_budget -= 1;
            if bad_partition_budget == 0 {
                heapsort_by_less(slice, is_less.as_ref());
                break;
            }

            break_patterns_by_less(slice, partition.left_len, partition.equal_len);
        } else if partition.already_partitioned {
            let left_sorted = left_len <= 1
                || partial_insertion_sort_by_less(&mut slice[..left_len], is_less.as_ref());
            let right_sorted = right_len <= 1
                || partial_insertion_sort_by_less(
                    &mut slice[right_begin - begin..],
                    is_less.as_ref(),
                );

            if left_sorted && right_sorted {
                break;
            }
        }

        if left_len <= sequential_cutoff && right_len <= sequential_cutoff {
            if left_len > 1 {
                unsafe {
                    sort_range_by_less(
                        data,
                        left_begin,
                        left_end,
                        left_is_leftmost,
                        is_less.as_ref(),
                    );
                }
            }
            if right_len > 1 {
                unsafe {
                    sort_range_by_less(
                        data,
                        right_begin,
                        right_end,
                        right_is_leftmost,
                        is_less.as_ref(),
                    );
                }
            }
            break;
        }

        let (spawn_begin, spawn_end, spawn_leftmost, inline_begin, inline_end, inline_leftmost) =
            if left_len <= right_len {
                (
                    left_begin,
                    left_end,
                    left_is_leftmost,
                    right_begin,
                    right_end,
                    right_is_leftmost,
                )
            } else {
                (
                    right_begin,
                    right_end,
                    right_is_leftmost,
                    left_begin,
                    left_end,
                    left_is_leftmost,
                )
            };
        let spawn_len = spawn_end.saturating_sub(spawn_begin);
        let inline_len = inline_end.saturating_sub(inline_begin);

        if spawn_len > 1 {
            let spawned_is_less = Arc::clone(&is_less);
            runtime.silent_async(move |runtime| unsafe {
                parallel_quicksort(
                    runtime,
                    data,
                    spawn_begin,
                    spawn_end,
                    spawned_is_less,
                    sequential_cutoff,
                    bad_partition_budget,
                    spawn_leftmost,
                )
                .expect("parallel sort runtime child should succeed");
            });
        }

        if inline_len <= 1 {
            break;
        }

        begin = inline_begin;
        end = inline_end;
        leftmost = inline_leftmost;
    }

    runtime.corun_children()
}

fn wait_all<T>(handles: Vec<AsyncHandle<T>>) -> Result<Vec<T>, FlowError> {
    handles.into_iter().map(AsyncHandle::wait).collect()
}
