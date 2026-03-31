use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flow_core::{Executor, Flow};

#[test]
fn composed_flow_joins_before_parent_successors_run() {
    let executor = Executor::new(2);
    let parent = Flow::new();
    let module = Flow::new();
    let events = Arc::new(Mutex::new(Vec::new()));

    let mod_a = {
        let events = Arc::clone(&events);
        module.spawn(move || {
            events.lock().expect("events poisoned").push("mod-a");
        })
    };
    let mod_b = {
        let events = Arc::clone(&events);
        module.spawn(move || {
            events.lock().expect("events poisoned").push("mod-b");
        })
    };
    mod_a.precede([mod_b.clone()]);

    let start = {
        let events = Arc::clone(&events);
        parent.spawn(move || {
            events.lock().expect("events poisoned").push("start");
        })
    };
    let composed = parent.compose(&module);
    let finish = {
        let events = Arc::clone(&events);
        parent.spawn(move || {
            let events = events.lock().expect("events poisoned");
            assert_eq!(*events, vec!["start", "mod-a", "mod-b"]);
        })
    };

    start.precede([composed.clone()]);
    composed.precede([finish]);

    executor
        .run(&parent)
        .wait()
        .expect("composed flow should succeed");
}

#[test]
fn composed_flow_can_be_reused_without_mutating_the_template() {
    let executor = Executor::new(2);
    let parent = Flow::new();
    let template = Flow::new();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let hits = Arc::clone(&hits);
        template.spawn(move || {
            hits.fetch_add(1, Ordering::SeqCst);
        });
    }

    let first = parent.compose(&template);
    let second = parent.compose(&template);
    first.precede([second.clone()]);

    executor
        .run(&parent)
        .wait()
        .expect("first composed run should succeed");
    executor
        .run(&parent)
        .wait()
        .expect("second composed run should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 4);
    assert_eq!(template.num_tasks(), 1);
}
