use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use flow_core::{AsyncTask, Executor};

#[test]
fn dependent_async_waits_for_all_predecessors() {
    let executor = Executor::new(1);
    let left = Arc::new(AtomicUsize::new(0));
    let right = Arc::new(AtomicUsize::new(0));

    let a = {
        let left = Arc::clone(&left);
        executor.silent_dependent_async(
            move || {
                left.store(10, Ordering::SeqCst);
            },
            std::iter::empty::<AsyncTask>(),
        )
    };

    let b = {
        let right = Arc::clone(&right);
        executor.silent_dependent_async(
            move || {
                right.store(32, Ordering::SeqCst);
            },
            std::iter::empty::<AsyncTask>(),
        )
    };

    let (_sum_task, handle) = {
        let left = Arc::clone(&left);
        let right = Arc::clone(&right);
        executor.dependent_async(
            move || {
                assert_eq!(left.load(Ordering::SeqCst), 10);
                assert_eq!(right.load(Ordering::SeqCst), 32);
                left.load(Ordering::SeqCst) + right.load(Ordering::SeqCst)
            },
            [a.clone(), b.clone()],
        )
    };

    assert_eq!(handle.wait().expect("dependent async should succeed"), 42);
}

#[test]
fn silent_dependent_async_supports_linear_chains() {
    let executor = Executor::new(2);
    let results = Arc::new((0..32).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    let mut previous = {
        let results = Arc::clone(&results);
        executor.silent_dependent_async(
            move || {
                results[0].store(1, Ordering::SeqCst);
            },
            std::iter::empty::<AsyncTask>(),
        )
    };

    for index in 1..32 {
        let results = Arc::clone(&results);
        previous = executor.silent_dependent_async(
            move || {
                let next = results[index - 1].load(Ordering::SeqCst) + index;
                results[index].store(next, Ordering::SeqCst);
            },
            [previous.clone()],
        );
    }

    executor.wait_for_all();

    assert_eq!(results[0].load(Ordering::SeqCst), 1);
    for index in 1..32 {
        assert_eq!(
            results[index].load(Ordering::SeqCst),
            results[index - 1].load(Ordering::SeqCst) + index
        );
    }
}

#[test]
fn empty_async_task_dependencies_are_ignored() {
    let executor = Executor::new(1);
    let hit = Arc::new(AtomicBool::new(false));

    {
        let hit = Arc::clone(&hit);
        executor.silent_dependent_async(
            move || {
                hit.store(true, Ordering::SeqCst);
            },
            [AsyncTask::default()],
        );
    }

    executor.wait_for_all();

    assert!(hit.load(Ordering::SeqCst));
}
