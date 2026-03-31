use std::sync::Arc;

use flow_algorithms::{
    Executor, ParallelForOptions, parallel_exclusive_scan, parallel_find, parallel_inclusive_scan,
    parallel_sort, parallel_sort_by,
};

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
