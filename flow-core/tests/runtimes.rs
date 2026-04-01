use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use flow_core::{Executor, Flow, RuntimeCtx};

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

fn runtime_fibonacci(runtime: &RuntimeCtx, n: usize) -> usize {
    if n < 2 {
        return n;
    }

    let left = runtime
        .executor()
        .runtime_async(move |runtime| runtime_fibonacci(runtime, n - 1));
    let right = runtime
        .executor()
        .runtime_async(move |runtime| runtime_fibonacci(runtime, n - 2));

    runtime.wait_async(left).expect("left runtime async should succeed")
        + runtime
            .wait_async(right)
            .expect("right runtime async should succeed")
}

#[test]
fn runtime_async_can_wait_inline_on_a_single_worker() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let result = Arc::new(Mutex::new(None));

    {
        let result = Arc::clone(&result);
        flow.spawn_runtime(move |runtime| {
            *result.lock().expect("result slot poisoned") = Some(runtime_fibonacci(runtime, 10));
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("runtime async recursion should succeed");

    assert_eq!(
        result.lock().expect("result slot poisoned").take(),
        Some(55)
    );
}

#[test]
fn runtime_wait_async_surfaces_child_panics() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let saw_error = Arc::new(AtomicBool::new(false));

    {
        let saw_error = Arc::clone(&saw_error);
        flow.spawn_runtime(move |runtime| {
            let handle = runtime.executor().runtime_async(|_| -> usize {
                panic!("runtime async child boom");
            });
            let error = runtime
                .wait_async(handle)
                .expect_err("runtime async child panic should surface");
            assert!(error.message().contains("runtime async child boom"));
            saw_error.store(true, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parent runtime flow should succeed when panic is handled");

    assert!(saw_error.load(Ordering::SeqCst));
}

#[test]
fn runtime_corun_handles_waits_for_all_children() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let hits = Arc::clone(&hits);
        flow.spawn_runtime(move |runtime| {
            let handles = (0..4)
                .map(|_| {
                    let hits = Arc::clone(&hits);
                    runtime.executor().runtime_silent_async(move |_| {
                        hits.fetch_add(1, Ordering::SeqCst);
                    })
                })
                .collect::<Vec<_>>();

            runtime
                .corun_handles(&handles)
                .expect("runtime corun_handles should succeed");
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parent runtime flow should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 4);
}

#[test]
fn runtime_corun_handles_surfaces_child_panics() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let saw_error = Arc::new(AtomicBool::new(false));

    {
        let saw_error = Arc::clone(&saw_error);
        flow.spawn_runtime(move |runtime| {
            let handles = vec![runtime.executor().runtime_silent_async(|_| {
                panic!("runtime silent async child boom");
            })];
            let error = runtime
                .corun_handles(&handles)
                .expect_err("runtime corun_handles child panic should surface");
            assert!(error.message().contains("runtime silent async child boom"));
            saw_error.store(true, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parent runtime flow should succeed when panic is handled");

    assert!(saw_error.load(Ordering::SeqCst));
}
