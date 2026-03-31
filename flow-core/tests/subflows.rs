use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use flow_core::{Executor, Flow};

#[test]
fn subflow_joins_before_parent_successors_run() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let subtask_hits = Arc::new(AtomicUsize::new(0));
    let successor_seen = Arc::new(AtomicBool::new(false));

    let start = flow.spawn(|| {}).name("start");
    let parent = {
        let subtask_hits = Arc::clone(&subtask_hits);
        flow.spawn_subflow(move |subflow| {
            let a = {
                let subtask_hits = Arc::clone(&subtask_hits);
                subflow.spawn(move || {
                    subtask_hits.fetch_add(1, Ordering::SeqCst);
                })
            };
            let b = {
                let subtask_hits = Arc::clone(&subtask_hits);
                subflow.spawn(move || {
                    subtask_hits.fetch_add(1, Ordering::SeqCst);
                })
            };
            let c = {
                let subtask_hits = Arc::clone(&subtask_hits);
                subflow.spawn(move || {
                    assert_eq!(subtask_hits.load(Ordering::SeqCst), 2);
                })
            };

            a.precede([c.clone()]);
            b.precede([c]);
        })
    }
    .name("parent");
    let done = {
        let subtask_hits = Arc::clone(&subtask_hits);
        let successor_seen = Arc::clone(&successor_seen);
        flow.spawn(move || {
            assert_eq!(subtask_hits.load(Ordering::SeqCst), 2);
            successor_seen.store(true, Ordering::SeqCst);
        })
    }
    .name("done");

    start.precede([parent.clone()]);
    done.succeed([parent]);

    executor
        .run(&flow)
        .wait()
        .expect("subflow run should succeed");

    assert!(successor_seen.load(Ordering::SeqCst));
}

#[test]
fn nested_subflows_join_recursively() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let nested_hits = Arc::new(AtomicUsize::new(0));

    let _parent = {
        let nested_hits = Arc::clone(&nested_hits);
        flow.spawn_subflow(move |subflow| {
            let nested_parent = {
                let nested_hits = Arc::clone(&nested_hits);
                subflow.spawn_subflow(move |nested| {
                    let leaf_a = {
                        let nested_hits = Arc::clone(&nested_hits);
                        nested.spawn(move || {
                            nested_hits.fetch_add(1, Ordering::SeqCst);
                        })
                    };
                    let leaf_b = {
                        let nested_hits = Arc::clone(&nested_hits);
                        nested.spawn(move || {
                            nested_hits.fetch_add(1, Ordering::SeqCst);
                        })
                    };
                    leaf_a.precede([leaf_b]);
                })
            };
            let tail = {
                let nested_hits = Arc::clone(&nested_hits);
                subflow.spawn(move || {
                    assert_eq!(nested_hits.load(Ordering::SeqCst), 2);
                })
            };

            nested_parent.precede([tail]);
        })
    };

    executor
        .run(&flow)
        .wait()
        .expect("nested subflow run should succeed");

    assert_eq!(nested_hits.load(Ordering::SeqCst), 2);
}
