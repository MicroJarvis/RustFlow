use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flow_core::{Executor, Flow};

#[test]
fn single_task_executes() {
    let executor = Executor::new(2);
    let flow = Flow::with_name("single");
    let counter = Arc::new(AtomicUsize::new(0));

    {
        let counter = Arc::clone(&counter);
        flow.spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor.run(&flow).wait().expect("run should succeed");

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn linear_dependencies_respected() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let ran_a = Arc::new(AtomicBool::new(false));

    let a = {
        let ran_a = Arc::clone(&ran_a);
        flow.spawn(move || {
            ran_a.store(true, Ordering::SeqCst);
        })
    }
    .name("A");

    let b = {
        let ran_a = Arc::clone(&ran_a);
        flow.spawn(move || {
            assert!(
                ran_a.load(Ordering::SeqCst),
                "B must observe that A ran first"
            );
        })
    }
    .name("B");

    a.precede([b.clone()]);

    executor.run(&flow).wait().expect("run should succeed");
}

#[test]
fn fork_join_executes_all_tasks() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));

    let a = {
        let hits = Arc::clone(&hits);
        flow.spawn(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        })
    };
    let b = {
        let hits = Arc::clone(&hits);
        let seen = Arc::clone(&seen);
        flow.spawn(move || {
            hits.fetch_add(1, Ordering::SeqCst);
            seen.lock().expect("seen poisoned").push("b");
        })
    };
    let c = {
        let hits = Arc::clone(&hits);
        let seen = Arc::clone(&seen);
        flow.spawn(move || {
            hits.fetch_add(1, Ordering::SeqCst);
            seen.lock().expect("seen poisoned").push("c");
        })
    };
    let d = {
        let hits = Arc::clone(&hits);
        let seen = Arc::clone(&seen);
        flow.spawn(move || {
            hits.fetch_add(1, Ordering::SeqCst);
            let seen = seen.lock().expect("seen poisoned");
            assert_eq!(seen.len(), 2, "join task should run after both branches");
        })
    };

    a.precede([b.clone(), c.clone()]);
    d.succeed([b, c]);

    executor.run(&flow).wait().expect("run should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 4);
}

#[test]
fn repeated_runs_reuse_the_same_graph() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let counter = Arc::new(AtomicUsize::new(0));

    {
        let counter = Arc::clone(&counter);
        flow.spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("first run should succeed");
    executor
        .run(&flow)
        .wait()
        .expect("second run should succeed");

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn graph_mutation_after_a_run_is_visible_to_the_next_snapshot() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let counter = Arc::new(AtomicUsize::new(0));

    {
        let counter = Arc::clone(&counter);
        flow.spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("first run should succeed");

    {
        let counter = Arc::clone(&counter);
        flow.spawn(move || {
            counter.fetch_add(10, Ordering::SeqCst);
        });
    }

    executor
        .run(&flow)
        .wait()
        .expect("second run should observe the mutated graph");

    assert_eq!(counter.load(Ordering::SeqCst), 12);
}

#[test]
fn run_n_executes_n_times() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let counter = Arc::new(AtomicUsize::new(0));

    {
        let counter = Arc::clone(&counter);
        flow.spawn(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor
        .run_n(&flow, 5)
        .wait()
        .expect("run_n should succeed");

    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

#[test]
fn run_until_stops_after_predicate_turns_true() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let task_hits = Arc::new(AtomicUsize::new(0));
    let stop_counter = Arc::new(AtomicUsize::new(0));

    {
        let task_hits = Arc::clone(&task_hits);
        flow.spawn(move || {
            task_hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    let stop_counter_for_predicate = Arc::clone(&stop_counter);
    executor
        .run_until(&flow, move || {
            stop_counter_for_predicate.fetch_add(1, Ordering::SeqCst) >= 2
        })
        .wait()
        .expect("run_until should succeed");

    assert_eq!(task_hits.load(Ordering::SeqCst), 3);
}

#[test]
fn panic_is_reported_to_the_run_handle() {
    let executor = Executor::new(2);
    let flow = Flow::new();

    flow.spawn(|| panic!("boom")).name("panic-task");

    let error = executor
        .run(&flow)
        .wait()
        .expect_err("run should report task panic");

    assert_eq!(error.task_name(), Some("panic-task"));
    assert!(error.message().contains("boom"));
}

#[test]
fn wait_for_all_blocks_until_all_runs_finish() {
    let executor = Executor::new(4);
    let flow = Flow::new();
    let counter = Arc::new(AtomicUsize::new(0));

    {
        let counter = Arc::clone(&counter);
        flow.spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    let _run1 = executor.run(&flow);
    let _run2 = executor.run(&flow);
    executor.wait_for_all();

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn dump_dot_contains_named_edges() {
    let flow = Flow::with_name("demo");
    let a = flow.spawn(|| {}).name("A");
    let b = flow.spawn(|| {}).name("B");
    a.precede([b]);

    let dot = flow.dump_dot();

    assert!(dot.contains("digraph flow"));
    assert!(dot.contains("label=\"demo\""));
    assert!(dot.contains("label=\"A\""));
    assert!(dot.contains("label=\"B\""));
    assert!(dot.contains("0 -> 1"));
}

#[test]
fn placeholder_tasks_participate_in_dependencies() {
    let executor = Executor::new(2);
    let flow = Flow::new();
    let hit = Arc::new(AtomicBool::new(false));

    let placeholder = flow.placeholder().name("placeholder");
    let tail = {
        let hit = Arc::clone(&hit);
        flow.spawn(move || {
            hit.store(true, Ordering::SeqCst);
        })
    };

    placeholder.precede([tail]);
    executor
        .run(&flow)
        .wait()
        .expect("placeholder run should succeed");

    assert!(hit.load(Ordering::SeqCst));
}

#[test]
fn parallel_fanout_executes_every_task_once() {
    let executor = Executor::new(8);
    let flow = Flow::new();
    let hits = Arc::new((0..512).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let root = flow.placeholder();

    let mut children = Vec::new();
    for index in 0..512 {
        let hits = Arc::clone(&hits);
        let task = flow.spawn(move || {
            hits[index].fetch_add(1, Ordering::SeqCst);
        });
        children.push(task);
    }

    root.precede(children);
    executor
        .run(&flow)
        .wait()
        .expect("parallel fanout run should succeed");

    for counter in hits.iter() {
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn run_waits_for_all_root_tasks_before_completing() {
    let executor = Executor::new(8);

    for _ in 0..32 {
        let flow = Flow::new();
        let hits = Arc::new((0..1024).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

        for index in 0..1024 {
            let hits = Arc::clone(&hits);
            flow.spawn(move || {
                hits[index].fetch_add(1, Ordering::SeqCst);
            });
        }

        executor
            .run(&flow)
            .wait()
            .expect("root fanout run should succeed");

        for hit in hits.iter() {
            assert_eq!(hit.load(Ordering::SeqCst), 1);
        }
    }
}
