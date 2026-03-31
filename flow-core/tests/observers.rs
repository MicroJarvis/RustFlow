use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flow_core::{Executor, Flow, Observer, TaskId};

#[derive(Default)]
struct RecordingObserver {
    started: Mutex<Vec<String>>,
    finished: Mutex<Vec<String>>,
    sleeps: AtomicUsize,
    wakes: AtomicUsize,
}

impl Observer for RecordingObserver {
    fn task_started(&self, _worker_id: usize, _task_id: TaskId, task_name: Option<&str>) {
        self.started
            .lock()
            .expect("started poisoned")
            .push(task_name.unwrap_or("").to_string());
    }

    fn task_finished(&self, _worker_id: usize, _task_id: TaskId, task_name: Option<&str>) {
        self.finished
            .lock()
            .expect("finished poisoned")
            .push(task_name.unwrap_or("").to_string());
    }

    fn worker_sleep(&self, _worker_id: usize) {
        self.sleeps.fetch_add(1, Ordering::SeqCst);
    }

    fn worker_wake(&self, _worker_id: usize) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn observer_receives_task_and_worker_lifecycle_events() {
    let executor = Executor::new(2);
    let observer = Arc::new(RecordingObserver::default());
    executor.add_observer(observer.clone());

    let flow = Flow::new();
    let a = flow
        .spawn(|| {
            std::thread::sleep(Duration::from_millis(10));
        })
        .name("A");
    let b = flow.spawn(|| {}).name("B");
    a.precede([b]);

    executor
        .run(&flow)
        .wait()
        .expect("observer flow should succeed");

    std::thread::sleep(Duration::from_millis(20));

    let started = observer.started.lock().expect("started poisoned").clone();
    let finished = observer.finished.lock().expect("finished poisoned").clone();

    assert_eq!(started.len(), 2);
    assert_eq!(finished.len(), 2);
    assert!(started.iter().any(|name| name == "A"));
    assert!(started.iter().any(|name| name == "B"));
    assert!(finished.iter().any(|name| name == "A"));
    assert!(finished.iter().any(|name| name == "B"));
    assert!(observer.sleeps.load(Ordering::SeqCst) > 0);
    assert!(observer.wakes.load(Ordering::SeqCst) > 0);
}
