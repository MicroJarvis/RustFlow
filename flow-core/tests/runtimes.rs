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

    runtime
        .wait_async(left)
        .expect("left runtime async should succeed")
        + runtime
            .wait_async(right)
            .expect("right runtime async should succeed")
}

fn task_group_fibonacci(executor: Executor, n: usize) -> usize {
    if n < 2 {
        return n;
    }

    let tg = executor.task_group();
    let left_executor = executor.clone();
    let left_slot = Arc::new(Mutex::new(None));
    {
        let left_slot = Arc::clone(&left_slot);
        tg.silent_async(move || {
            *left_slot.lock().expect("task-group left slot poisoned") =
                Some(task_group_fibonacci(left_executor, n - 1));
        });
    }
    let right = task_group_fibonacci(executor.clone(), n - 2);
    tg.corun()
        .expect("task-group fibonacci child should succeed");
    left_slot
        .lock()
        .expect("task-group left slot poisoned")
        .take()
        .expect("task-group left slot should be initialized")
        + right
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
fn executor_task_group_can_wait_inline_on_a_single_worker() {
    let executor = Executor::new(1);
    let root_executor = executor.clone();

    let result = executor
        .async_task(move || task_group_fibonacci(root_executor, 10))
        .wait()
        .expect("task-group recursion should succeed");

    assert_eq!(result, 55);
}

#[test]
fn executor_task_group_can_wait_from_an_external_thread() {
    let executor = Executor::new(2);
    let tg = executor.task_group();
    let hits = Arc::new(AtomicUsize::new(0));

    for _ in 0..4 {
        let hits = Arc::clone(&hits);
        tg.silent_async(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    tg.corun()
        .expect("external-thread task-group wait should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 4);
}

#[test]
fn executor_task_group_corun_is_a_noop_without_children() {
    let executor = Executor::new(2);
    let tg = executor.task_group();

    assert_eq!(tg.size(), 0);
    tg.corun()
        .expect("empty task-group corun should succeed immediately");
    assert_eq!(tg.size(), 0);
}

#[test]
fn executor_task_group_cancel_before_spawn_skips_children() {
    let executor = Executor::new(2);
    let tg = executor.task_group();
    let hits = Arc::new(AtomicUsize::new(0));

    tg.cancel();
    assert!(tg.is_cancelled());

    {
        let hits = Arc::clone(&hits);
        tg.silent_async(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    tg.corun()
        .expect("cancelled task-group corun should still complete cleanly");

    assert_eq!(tg.size(), 0);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn executor_task_group_is_tracked_by_wait_for_all_without_corun() {
    let executor = Executor::new(2);
    let tg = executor.task_group();
    let hits = Arc::new(AtomicUsize::new(0));

    for _ in 0..4 {
        let hits = Arc::clone(&hits);
        tg.silent_async(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor.wait_for_all();

    assert_eq!(hits.load(Ordering::SeqCst), 4);
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
fn runtime_async_child_can_observe_parent_cancellation() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let child_reached = Arc::new(AtomicBool::new(false));
    let child_cancelled = Arc::new(AtomicBool::new(false));

    {
        let child_reached = Arc::clone(&child_reached);
        let child_cancelled = Arc::clone(&child_cancelled);
        flow.spawn_runtime(move |runtime| {
            let child_reached = Arc::clone(&child_reached);
            let child_cancelled = Arc::clone(&child_cancelled);
            let handle = runtime.executor().runtime_async(move |runtime| {
                child_reached.store(true, Ordering::SeqCst);
                while !runtime.is_cancelled() {
                    std::thread::yield_now();
                }
                child_cancelled.store(true, Ordering::SeqCst);
            });

            runtime
                .wait_async(handle)
                .expect("runtime async child should stop after parent cancellation");
        });
    }

    let handle = executor.run(&flow);
    while !child_reached.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }

    assert!(handle.cancel(), "parent runtime flow should be cancellable");
    handle
        .wait()
        .expect("cancelled parent runtime flow should drain cleanly");

    assert!(child_cancelled.load(Ordering::SeqCst));
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
fn runtime_corun_children_waits_for_all_silent_children() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let hits = Arc::clone(&hits);
        flow.spawn_runtime(move |runtime| {
            for _ in 0..4 {
                let hits = Arc::clone(&hits);
                runtime.silent_async(move |_| {
                    hits.fetch_add(1, Ordering::SeqCst);
                });
            }

            runtime
                .corun_children()
                .expect("runtime corun_children should succeed");
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parent runtime flow should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 4);
}

#[test]
fn runtime_silent_children_are_tracked_by_wait_for_all_without_corun() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let hits = Arc::clone(&hits);
        flow.spawn_runtime(move |runtime| {
            for _ in 0..4 {
                let hits = Arc::clone(&hits);
                runtime.silent_async(move |_| {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    hits.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parent runtime flow should complete");

    executor.wait_for_all();

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

#[test]
fn runtime_corun_children_surfaces_child_panics() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let saw_error = Arc::new(AtomicBool::new(false));

    {
        let saw_error = Arc::clone(&saw_error);
        flow.spawn_runtime(move |runtime| {
            runtime.silent_async(|_| {
                panic!("runtime silent async child boom");
            });

            let error = runtime
                .corun_children()
                .expect_err("runtime corun_children child panic should surface");
            // After performance optimization, RuntimeJoinScope uses AtomicBool
            // for error tracking instead of storing the actual error message.
            // This reduces mutex overhead on every child completion.
            assert!(error.message().contains("runtime child task failed"));
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
fn task_group_corun_surfaces_child_panics() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let saw_error = Arc::new(AtomicBool::new(false));

    {
        let executor = executor.clone();
        let saw_error = Arc::clone(&saw_error);
        flow.spawn_runtime(move |_| {
            let tg = executor.task_group();
            tg.silent_async(|| {
                panic!("task group child boom");
            });

            let error = tg
                .corun()
                .expect_err("task-group child panic should surface");
            assert!(error.message().contains("task in task group panicked"));
            saw_error.store(true, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("parent runtime flow should succeed when task-group panic is handled");

    assert!(saw_error.load(Ordering::SeqCst));
}

#[test]
fn runtime_corun_children_does_not_deadlock_under_repeated_batches() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let hits = Arc::clone(&hits);
        flow.spawn_runtime(move |runtime| {
            for _ in 0..32 {
                for _ in 0..8 {
                    let hits = Arc::clone(&hits);
                    runtime.silent_async(move |_| {
                        hits.fetch_add(1, Ordering::SeqCst);
                    });
                }

                runtime
                    .corun_children()
                    .expect("repeated runtime corun_children should succeed");
            }
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("repeated runtime batches should complete");

    assert_eq!(hits.load(Ordering::SeqCst), 32 * 8);
}
