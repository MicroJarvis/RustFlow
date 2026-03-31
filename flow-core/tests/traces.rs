use std::sync::Arc;
use std::time::Duration;

use flow_core::{Executor, Flow, TraceObserver};

#[test]
fn trace_observer_exports_task_spans_as_timeline_json() {
    let executor = Executor::new(2);
    let observer = Arc::new(TraceObserver::new());
    executor.add_observer(observer.clone());

    let flow = Flow::new();
    let a = flow
        .spawn(|| {
            std::thread::sleep(Duration::from_millis(5));
        })
        .name("A");
    let b = flow
        .spawn(|| {
            std::thread::sleep(Duration::from_millis(5));
        })
        .name("B");
    a.precede([b]);

    executor
        .run(&flow)
        .wait()
        .expect("trace flow should succeed");

    let trace = observer.dump();

    assert_eq!(observer.num_tasks(), 2);
    assert!(trace.starts_with('['));
    assert!(trace.contains("\"cat\":\"TraceObserver\""));
    assert!(trace.contains("\"ph\":\"X\""));
    assert!(trace.contains("\"name\":\"A\""));
    assert!(trace.contains("\"name\":\"B\""));
    assert!(trace.contains("\"tid\":"));
    assert!(trace.contains("\"ts\":"));
    assert!(trace.contains("\"dur\":"));
}

#[test]
fn trace_observer_clear_discards_previous_spans() {
    let executor = Executor::new(2);
    let observer = Arc::new(TraceObserver::new());
    executor.add_observer(observer.clone());

    let flow = Flow::new();
    flow.spawn(|| {}).name("once");

    executor
        .run(&flow)
        .wait()
        .expect("trace flow should succeed");

    assert_eq!(observer.num_tasks(), 1);

    observer.clear();

    assert_eq!(observer.num_tasks(), 0);
    assert_eq!(observer.dump().trim(), "[]");
}
