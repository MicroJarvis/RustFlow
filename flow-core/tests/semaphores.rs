use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use flow_core::{Executor, Flow, Semaphore};

#[test]
fn semaphore_limits_task_concurrency() {
    let executor = Executor::new(8);
    let flow = Flow::new();
    let semaphore = Semaphore::new(3);
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    for _ in 0..64 {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        flow.spawn(move || {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = peak.load(Ordering::SeqCst);
            while current > observed {
                match peak.compare_exchange(observed, current, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => break,
                    Err(actual) => observed = actual,
                }
            }

            std::thread::sleep(Duration::from_millis(5));
            active.fetch_sub(1, Ordering::SeqCst);
        })
        .acquire(&semaphore)
        .release(&semaphore);
    }

    executor
        .run(&flow)
        .wait()
        .expect("first semaphore run should succeed");
    executor
        .run(&flow)
        .wait()
        .expect("second semaphore run should succeed");

    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert!(peak.load(Ordering::SeqCst) <= 3);
    assert_eq!(semaphore.value(), 3);
    assert_eq!(semaphore.max_value(), 3);
}
