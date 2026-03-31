use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use flow_core::Executor;

#[test]
fn async_task_returns_a_value() {
    let executor = Executor::new(2);

    let handle = executor.async_task(|| 21 * 2);

    assert_eq!(handle.wait().expect("async task should succeed"), 42);
}

#[test]
fn async_task_reports_panics_through_its_handle() {
    let executor = Executor::new(2);

    let error = executor
        .async_task(|| -> usize { panic!("async boom") })
        .wait()
        .expect_err("async panic should be reported");

    assert!(error.message().contains("async boom"));
}

#[test]
fn silent_async_is_tracked_by_wait_for_all() {
    let executor = Executor::new(4);
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..16 {
        let counter = Arc::clone(&counter);
        executor.silent_async(move || {
            std::thread::sleep(Duration::from_millis(10));
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    executor.wait_for_all();

    assert_eq!(counter.load(Ordering::SeqCst), 16);
}
