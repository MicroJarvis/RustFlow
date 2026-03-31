use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flow_core::{Executor, Flow};

#[test]
fn condition_selects_exactly_one_successor() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let left_hits = Arc::new(AtomicUsize::new(0));
    let right_hits = Arc::new(AtomicUsize::new(0));

    let init = flow.spawn(|| {});
    let condition = flow.spawn_condition(|| 1);
    let left = {
        let left_hits = Arc::clone(&left_hits);
        flow.spawn(move || {
            left_hits.fetch_add(1, Ordering::SeqCst);
        })
    };
    let right = {
        let right_hits = Arc::clone(&right_hits);
        flow.spawn(move || {
            right_hits.fetch_add(1, Ordering::SeqCst);
        })
    };

    init.precede([condition.clone()]);
    condition.precede([left, right]);

    executor
        .run(&flow)
        .wait()
        .expect("condition flow should succeed");

    assert_eq!(left_hits.load(Ordering::SeqCst), 0);
    assert_eq!(right_hits.load(Ordering::SeqCst), 1);
}

#[test]
fn multi_condition_can_select_multiple_successors() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let hits = Arc::new((0..3).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    let init = flow.spawn(|| {});
    let condition = flow.spawn_multi_condition(|| vec![0, 2]);

    let mut branches = Vec::new();
    for index in 0..3 {
        let hits = Arc::clone(&hits);
        branches.push(flow.spawn(move || {
            hits[index].fetch_add(1, Ordering::SeqCst);
        }));
    }

    init.precede([condition.clone()]);
    condition.precede(branches);

    executor
        .run(&flow)
        .wait()
        .expect("multi-condition flow should succeed");

    assert_eq!(hits[0].load(Ordering::SeqCst), 1);
    assert_eq!(hits[1].load(Ordering::SeqCst), 0);
    assert_eq!(hits[2].load(Ordering::SeqCst), 1);
}

#[test]
fn condition_feedback_loop_repeats_until_exit_branch() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let iterations = Arc::new(AtomicUsize::new(0));

    let init = flow.spawn(|| {});
    let condition = {
        let iterations = Arc::clone(&iterations);
        flow.spawn_condition(move || {
            if iterations.fetch_add(1, Ordering::SeqCst) < 2 {
                0
            } else {
                1
            }
        })
    };
    let done = flow.spawn(|| {});

    init.precede([condition.clone()]);
    condition.precede([condition.clone(), done]);

    executor
        .run(&flow)
        .wait()
        .expect("feedback loop flow should succeed");

    assert_eq!(iterations.load(Ordering::SeqCst), 3);
}
