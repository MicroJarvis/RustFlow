use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use flow_core::{Executor, Flow};

#[test]
fn cancelling_a_run_stops_unactivated_successors() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let started = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let tail_ran = Arc::new(AtomicBool::new(false));

    let head = {
        let started = Arc::clone(&started);
        let release = Arc::clone(&release);
        flow.spawn(move || {
            started.store(true, Ordering::SeqCst);
            while !release.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
        })
    };

    let tail = {
        let tail_ran = Arc::clone(&tail_ran);
        flow.spawn(move || {
            tail_ran.store(true, Ordering::SeqCst);
        })
    };

    head.precede([tail]);

    let handle = executor.run(&flow);
    while !started.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }

    assert!(handle.cancel(), "run should be cancellable while active");
    release.store(true, Ordering::SeqCst);

    handle
        .wait()
        .expect("cancelled run should still drain cleanly");

    assert!(!tail_ran.load(Ordering::SeqCst));
}

#[test]
fn runtime_task_can_observe_run_cancellation() {
    let executor = Executor::new(1);
    let flow = Flow::new();
    let reached = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));

    {
        let reached = Arc::clone(&reached);
        let cancelled = Arc::clone(&cancelled);
        flow.spawn_runtime(move |runtime| {
            reached.store(true, Ordering::SeqCst);
            while !cancelled.load(Ordering::SeqCst) {
                std::thread::yield_now();
                if runtime.is_cancelled() {
                    cancelled.store(true, Ordering::SeqCst);
                }
            }
        });
    }

    let handle = executor.run(&flow);
    while !reached.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }

    assert!(
        handle.cancel(),
        "run should be cancellable while runtime task is active"
    );
    handle
        .wait()
        .expect("cancelled runtime flow should drain cleanly");

    assert!(cancelled.load(Ordering::SeqCst));
}
