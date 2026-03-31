use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flow_algorithms::{Executor, Flow, ParallelForExt, ParallelForOptions};

#[test]
fn parallel_for_visits_each_index_once() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let hits = Arc::new((0..128).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    {
        let hits = Arc::clone(&hits);
        flow.parallel_for(0..128, ParallelForOptions::default(), move |index| {
            hits[index].fetch_add(1, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parallel_for flow should succeed");

    for hit in hits.iter() {
        assert_eq!(hit.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn parallel_for_respects_explicit_chunk_size() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let sum = Arc::new(AtomicUsize::new(0));

    let tasks = {
        let sum = Arc::clone(&sum);
        flow.parallel_for(
            0..10,
            ParallelForOptions::default().with_chunk_size(3),
            move |index| {
                sum.fetch_add(index, Ordering::SeqCst);
            },
        )
    };

    assert_eq!(tasks.len(), 4);
    assert_eq!(flow.num_tasks(), 4);

    executor
        .run(&flow)
        .wait()
        .expect("chunked parallel_for flow should succeed");

    assert_eq!(sum.load(Ordering::SeqCst), (0..10).sum());
}

#[test]
fn parallel_for_empty_range_creates_no_tasks() {
    let flow = Flow::new();

    let tasks = flow.parallel_for(0..0, ParallelForOptions::default(), |_| {
        panic!("empty range should not execute");
    });

    assert!(tasks.is_empty());
    assert!(flow.is_empty());
}
