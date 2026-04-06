use std::sync::Arc;

use flow_algorithms::{
    Executor, ParallelForOptions, parallel_exclusive_scan, parallel_find, parallel_inclusive_scan,
    parallel_sort, parallel_sort_by,
};
use rayon::slice::ParallelSliceMut;

const PARALLEL_SORT_TEST_LEN: usize = 200_000;

fn assert_parallel_sort_matches_oracles(input: Vec<i32>, options: ParallelForOptions) {
    let executor = Executor::new(4);
    let mut expected_std = input.clone();
    expected_std.sort_unstable();

    let mut expected_rayon = input.clone();
    expected_rayon.par_sort_unstable();

    let output = parallel_sort(&executor, input, options).expect("parallel sort should succeed");

    assert_eq!(output, expected_std);
    assert_eq!(output, expected_rayon);
}

fn assert_parallel_sort_by_matches_oracles<F>(
    input: Vec<i32>,
    options: ParallelForOptions,
    compare: F,
) where
    F: Fn(&i32, &i32) -> std::cmp::Ordering + Send + Sync + Copy + 'static,
{
    let executor = Executor::new(4);
    let mut expected_std = input.clone();
    expected_std.sort_unstable_by(compare);

    let mut expected_rayon = input.clone();
    expected_rayon.par_sort_unstable_by(compare);

    let output = parallel_sort_by(&executor, input, options, compare)
        .expect("parallel sort by should succeed");

    assert_eq!(output, expected_std);
    assert_eq!(output, expected_rayon);
}

#[test]
fn parallel_find_returns_first_matching_index() {
    let executor = Executor::new(4);
    let input: Arc<[usize]> = vec![9, 7, 5, 4, 8, 4, 2, 4, 1].into();

    let found = parallel_find(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default().with_chunk_size(2),
        |value| *value == 4,
    )
    .expect("parallel find should succeed");

    assert_eq!(found, Some(3));
}

#[test]
fn parallel_find_returns_none_when_not_found() {
    let executor = Executor::new(3);
    let input: Arc<[usize]> = (0..64).collect::<Vec<_>>().into();

    let found = parallel_find(
        &executor,
        input,
        ParallelForOptions::default().with_chunk_size(5),
        |value| *value == 128,
    )
    .expect("parallel find should succeed");

    assert_eq!(found, None);
}

#[test]
fn parallel_inclusive_scan_matches_sequential_result() {
    let executor = Executor::new(4);
    let input: Arc<[usize]> = (1..=32).collect::<Vec<_>>().into();

    let output = parallel_inclusive_scan(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default().with_chunk_size(4),
        |left, right| left + right,
    )
    .expect("parallel inclusive scan should succeed");

    let mut running = 0usize;
    let expected = input
        .iter()
        .map(|value| {
            running += value;
            running
        })
        .collect::<Vec<_>>();

    assert_eq!(output, expected);
}

#[test]
fn parallel_exclusive_scan_matches_sequential_result() {
    let executor = Executor::new(4);
    let input: Arc<[usize]> = (1..=16).collect::<Vec<_>>().into();

    let output = parallel_exclusive_scan(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default().with_chunk_size(3),
        10usize,
        |left, right| left + right,
    )
    .expect("parallel exclusive scan should succeed");

    let mut running = 10usize;
    let expected = input
        .iter()
        .map(|value| {
            let current = running;
            running += value;
            current
        })
        .collect::<Vec<_>>();

    assert_eq!(output, expected);
}

#[test]
fn parallel_inclusive_scan_large_matches_sequential_result() {
    let executor = Executor::new(4);
    let input: Arc<[u64]> = (1..=20_000).collect::<Vec<_>>().into();

    let output = parallel_inclusive_scan(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default(),
        |left, right| left + right,
    )
    .expect("large parallel inclusive scan should succeed");

    let mut running = 0u64;
    let expected = input
        .iter()
        .map(|value| {
            running += value;
            running
        })
        .collect::<Vec<_>>();

    assert_eq!(output, expected);
}

#[test]
fn parallel_exclusive_scan_large_matches_sequential_result() {
    let executor = Executor::new(4);
    let input: Arc<[u64]> = (1..=20_000).collect::<Vec<_>>().into();

    let output = parallel_exclusive_scan(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default(),
        7u64,
        |left, right| left + right,
    )
    .expect("large parallel exclusive scan should succeed");

    let mut running = 7u64;
    let expected = input
        .iter()
        .map(|value| {
            let current = running;
            running += value;
            current
        })
        .collect::<Vec<_>>();

    assert_eq!(output, expected);
}

#[test]
fn parallel_sort_matches_std_sort() {
    let executor = Executor::new(4);
    let input = vec![9, 3, 7, 1, 8, 2, 6, 5, 4, 0, 7, 3, 9, 2];
    let mut expected = input.clone();
    expected.sort_unstable();

    let output = parallel_sort(
        &executor,
        input,
        ParallelForOptions::default().with_chunk_size(3),
    )
    .expect("parallel sort should succeed");

    assert_eq!(output, expected);
}

#[test]
fn parallel_sort_by_supports_custom_order() {
    let executor = Executor::new(4);
    let input = vec![1, 4, 2, 8, 5, 7, 3, 6];
    let mut expected = input.clone();
    expected.sort_unstable_by(|left, right| right.cmp(left));

    let output = parallel_sort_by(
        &executor,
        input,
        ParallelForOptions::default().with_chunk_size(2),
        |left: &i32, right: &i32| right.cmp(left),
    )
    .expect("parallel sort by should succeed");

    assert_eq!(output, expected);
}

#[test]
fn parallel_sort_large_matches_std_sort() {
    let executor = Executor::new(4);
    let input = (0..50_000)
        .rev()
        .map(|index| ((index as u64 * 48_271 + 1_234_567) % 100_003) as i32)
        .collect::<Vec<_>>();
    let mut expected = input.clone();
    expected.sort_unstable();

    let output = parallel_sort(&executor, input, ParallelForOptions::default())
        .expect("large parallel sort should succeed");

    assert_eq!(output, expected);
}

#[test]
fn parallel_sort_preserves_sorted_input_on_parallel_path() {
    let input = (0..PARALLEL_SORT_TEST_LEN as i32).collect::<Vec<_>>();
    assert_parallel_sort_matches_oracles(input, ParallelForOptions::default());
}

#[test]
fn parallel_sort_handles_reverse_sorted_input_on_parallel_path() {
    let input = (0..PARALLEL_SORT_TEST_LEN as i32).rev().collect::<Vec<_>>();
    assert_parallel_sort_matches_oracles(input, ParallelForOptions::default());
}

#[test]
fn parallel_sort_handles_duplicate_heavy_input_on_parallel_path() {
    let input = (0..PARALLEL_SORT_TEST_LEN)
        .map(|index| (index % 11) as i32 - 5)
        .collect::<Vec<_>>();
    assert_parallel_sort_matches_oracles(input, ParallelForOptions::default());
}

#[test]
fn parallel_sort_handles_all_equal_input_on_parallel_path() {
    let input = vec![42; PARALLEL_SORT_TEST_LEN];
    assert_parallel_sort_matches_oracles(input, ParallelForOptions::default());
}

#[test]
fn parallel_sort_handles_sorted_duplicate_runs_on_parallel_path() {
    let input = (0..PARALLEL_SORT_TEST_LEN)
        .map(|index| (index / 97) as i32)
        .collect::<Vec<_>>();
    assert_parallel_sort_matches_oracles(input, ParallelForOptions::default());
}

#[test]
fn parallel_sort_handles_nearly_sorted_input_on_parallel_path() {
    let mut input = (0..PARALLEL_SORT_TEST_LEN as i32).collect::<Vec<_>>();
    for offset in (0..PARALLEL_SORT_TEST_LEN.saturating_sub(17)).step_by(4096) {
        input.swap(offset, offset + 17);
    }
    assert_parallel_sort_matches_oracles(input, ParallelForOptions::default());
}

#[test]
fn parallel_sort_by_matches_descending_oracles_on_parallel_path() {
    let input = (0..PARALLEL_SORT_TEST_LEN)
        .map(|index| ((index as i32 * 37) % 10_007) - 5_000)
        .collect::<Vec<_>>();
    assert_parallel_sort_by_matches_oracles(input, ParallelForOptions::default(), |left, right| {
        right.cmp(left)
    });
}

#[test]
fn parallel_sort_by_matches_descending_oracles_for_duplicate_heavy_input() {
    let input = (0..PARALLEL_SORT_TEST_LEN)
        .map(|index| (index % 9) as i32)
        .collect::<Vec<_>>();
    assert_parallel_sort_by_matches_oracles(input, ParallelForOptions::default(), |left, right| {
        right.cmp(left)
    });
}
