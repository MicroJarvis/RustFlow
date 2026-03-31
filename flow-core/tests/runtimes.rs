use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use flow_core::{Executor, Flow};

#[test]
fn runtime_task_can_access_executor_and_worker_context() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let seen_workers = Arc::new(Mutex::new(Vec::new()));

    {
        let seen_workers = Arc::clone(&seen_workers);
        flow.spawn_runtime(move |runtime| {
            assert_eq!(runtime.executor().num_workers(), 4);
            assert!(runtime.worker_id() < runtime.executor().num_workers());
            seen_workers
                .lock()
                .expect("seen_workers poisoned")
                .push(runtime.worker_id());
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("runtime flow should succeed");

    assert_eq!(seen_workers.lock().expect("seen_workers poisoned").len(), 1);
}

#[test]
fn runtime_task_is_available_inside_subflow() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let hit = Arc::new(AtomicBool::new(false));

    {
        let hit = Arc::clone(&hit);
        flow.spawn_subflow(move |subflow| {
            let hit = Arc::clone(&hit);
            subflow.spawn_runtime(move |runtime| {
                assert_eq!(runtime.executor().num_workers(), 2);
                assert!(runtime.worker_id() < 2);
                hit.store(true, Ordering::SeqCst);
            });
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("runtime subflow should succeed");

    assert!(hit.load(Ordering::SeqCst));
}

#[test]
fn runtime_task_can_schedule_an_active_task() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let scheduled_hits = Arc::new(AtomicUsize::new(0));
    let done_hits = Arc::new(AtomicUsize::new(0));

    let init = flow.spawn(|| {});
    let condition = flow.spawn_condition(|| 0);
    let scheduled = {
        let scheduled_hits = Arc::clone(&scheduled_hits);
        flow.spawn(move || {
            scheduled_hits.fetch_add(1, Ordering::SeqCst);
        })
    };
    let runtime = {
        let scheduled = scheduled.clone();
        flow.spawn_runtime(move |runtime| {
            runtime.schedule(scheduled.clone());
        })
    };
    let done = {
        let done_hits = Arc::clone(&done_hits);
        let scheduled_hits = Arc::clone(&scheduled_hits);
        flow.spawn(move || {
            assert_eq!(scheduled_hits.load(Ordering::SeqCst), 1);
            done_hits.fetch_add(1, Ordering::SeqCst);
        })
    };

    init.precede([condition.clone()]);
    condition.precede([runtime.clone(), scheduled.clone()]);
    done.succeed([runtime, scheduled]);

    executor
        .run(&flow)
        .wait()
        .expect("runtime schedule flow should succeed");

    assert_eq!(scheduled_hits.load(Ordering::SeqCst), 1);
    assert_eq!(done_hits.load(Ordering::SeqCst), 1);
}

#[test]
fn runtime_task_can_corun_another_flow_on_single_worker() {
    let executor = Executor::new(1);
    let parent = Flow::new();
    let child = Flow::new();
    let child_hits = Arc::new(AtomicUsize::new(0));

    {
        let child_hits = Arc::clone(&child_hits);
        child.spawn(move || {
            child_hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    {
        let child = child.clone();
        parent.spawn_runtime(move |runtime| {
            runtime.corun(&child).expect("corun should succeed");
        });
    }

    executor
        .run(&parent)
        .wait()
        .expect("parent flow should succeed");

    assert_eq!(child_hits.load(Ordering::SeqCst), 1);
}

#[test]
fn runtime_corun_surfaces_child_errors() {
    let executor = Executor::new(1);
    let parent = Flow::new();
    let child = Flow::new();

    child.spawn(|| panic!("child boom")).name("child-panic");

    {
        let child = child.clone();
        parent.spawn_runtime(move |runtime| {
            let error = runtime.corun(&child).expect_err("corun should fail");
            assert_eq!(error.task_name(), Some("child-panic"));
        });
    }

    executor
        .run(&parent)
        .wait()
        .expect("parent flow should still succeed when child error is handled");
}

#[test]
fn runtime_corun_supports_child_runtime_scheduling() {
    let executor = Executor::new(1);
    let parent = Flow::new();
    let child = Flow::new();
    let scheduled_hits = Arc::new(AtomicUsize::new(0));
    let done_hits = Arc::new(AtomicUsize::new(0));

    let init = child.spawn(|| {});
    let condition = child.spawn_condition(|| 0);
    let scheduled = {
        let scheduled_hits = Arc::clone(&scheduled_hits);
        child.spawn(move || {
            scheduled_hits.fetch_add(1, Ordering::SeqCst);
        })
    };
    let runtime = {
        let scheduled = scheduled.clone();
        child.spawn_runtime(move |runtime| {
            runtime.schedule(scheduled.clone());
        })
    };
    let done = {
        let done_hits = Arc::clone(&done_hits);
        let scheduled_hits = Arc::clone(&scheduled_hits);
        child.spawn(move || {
            assert_eq!(scheduled_hits.load(Ordering::SeqCst), 1);
            done_hits.fetch_add(1, Ordering::SeqCst);
        })
    };

    init.precede([condition.clone()]);
    condition.precede([runtime.clone(), scheduled.clone()]);
    done.succeed([runtime, scheduled]);

    {
        let child = child.clone();
        parent.spawn_runtime(move |runtime| {
            runtime.corun(&child).expect("corun should succeed");
        });
    }

    executor
        .run(&parent)
        .wait()
        .expect("parent flow should succeed");

    assert_eq!(scheduled_hits.load(Ordering::SeqCst), 1);
    assert_eq!(done_hits.load(Ordering::SeqCst), 1);
}
