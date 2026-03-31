use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flow_core::compat::{Executor, Flow};

#[test]
fn compat_flow_runs_static_tasks_and_dependencies() {
    let executor = Executor::new(2);
    let flow = Flow::with_name("compat");
    let hits = Arc::new(AtomicUsize::new(0));

    let a = {
        let hits = Arc::clone(&hits);
        flow.emplace(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        })
    }
    .name("A");
    let b = {
        let hits = Arc::clone(&hits);
        flow.emplace(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        })
    }
    .name("B");

    a.precede([b]);

    executor
        .run(&flow)
        .wait()
        .expect("compat flow run should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert_eq!(flow.num_tasks(), 2);
    assert!(flow.dump_dot().contains("label=\"compat\""));
}

#[test]
fn compat_flow_supports_condition_and_subflow_entry_points() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let branch_hits = Arc::new(AtomicUsize::new(0));
    let subflow_hits = Arc::new(AtomicUsize::new(0));

    let init = flow.emplace(|| {});
    let condition = flow.emplace_condition(|| 1);
    let left = {
        let branch_hits = Arc::clone(&branch_hits);
        flow.emplace(move || {
            branch_hits.fetch_add(10, Ordering::SeqCst);
        })
    };
    let right = {
        let branch_hits = Arc::clone(&branch_hits);
        flow.emplace(move || {
            branch_hits.fetch_add(1, Ordering::SeqCst);
        })
    };
    let subflow = {
        let subflow_hits = Arc::clone(&subflow_hits);
        flow.emplace_subflow(move |subflow| {
            let first = {
                let subflow_hits = Arc::clone(&subflow_hits);
                subflow.emplace(move || {
                    subflow_hits.fetch_add(1, Ordering::SeqCst);
                })
            };
            let second = {
                let subflow_hits = Arc::clone(&subflow_hits);
                subflow.emplace(move || {
                    subflow_hits.fetch_add(1, Ordering::SeqCst);
                })
            };
            first.precede([second]);
        })
    };

    init.precede([condition.clone(), subflow]);
    condition.precede([left, right]);

    executor
        .run(&flow)
        .wait()
        .expect("compat flow run should succeed");

    assert_eq!(branch_hits.load(Ordering::SeqCst), 1);
    assert_eq!(subflow_hits.load(Ordering::SeqCst), 2);
}

#[test]
fn compat_executor_forwards_run_n_and_wait_for_all() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let hits = Arc::clone(&hits);
        flow.emplace(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor
        .run_n(&flow, 3)
        .wait()
        .expect("compat run_n should succeed");
    executor.wait_for_all();

    assert_eq!(hits.load(Ordering::SeqCst), 3);
}
