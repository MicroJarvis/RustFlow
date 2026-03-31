use std::sync::Arc;

use flow_algorithms::{Executor, ParallelForOptions, parallel_reduce, parallel_transform};

#[test]
fn parallel_transform_produces_expected_values() {
    let executor = Executor::new(4);
    let input: Arc<[usize]> = (0..128).collect::<Vec<_>>().into();

    let output = parallel_transform(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default().with_chunk_size(8),
        |value| value * 2,
    )
    .expect("parallel transform should succeed");

    let expected = input.iter().map(|value| value * 2).collect::<Vec<_>>();
    assert_eq!(output, expected);
}

#[test]
fn parallel_reduce_produces_expected_sum() {
    let executor = Executor::new(4);
    let input: Arc<[usize]> = (1..=1000).collect::<Vec<_>>().into();

    let sum = parallel_reduce(
        &executor,
        Arc::clone(&input),
        ParallelForOptions::default().with_chunk_size(17),
        0usize,
        |left, right| left + right,
    )
    .expect("parallel reduce should succeed");

    assert_eq!(sum, input.iter().copied().sum());
}

#[test]
fn parallel_reduce_returns_init_for_empty_input() {
    let executor = Executor::new(2);
    let input: Arc<[usize]> = Vec::<usize>::new().into();

    let sum = parallel_reduce(
        &executor,
        input,
        ParallelForOptions::default(),
        42usize,
        |left, right| left + right,
    )
    .expect("empty parallel reduce should succeed");

    assert_eq!(sum, 42);
}
