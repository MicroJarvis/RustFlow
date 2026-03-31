use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flow_core::{Executor, Pipe, Pipeline};

#[test]
fn linear_pipeline_processes_tokens_until_stop() {
    let executor = Executor::new(4);
    let stage0 = Arc::new((0..6).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let stage1 = Arc::new((0..6).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let stage2 = Arc::new((0..6).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    let pipeline = Pipeline::from_pipes(
        4,
        vec![
            {
                let stage0 = Arc::clone(&stage0);
                Pipe::serial(move |ctx| {
                    if ctx.token() == 5 {
                        ctx.stop();
                        return;
                    }
                    stage0[ctx.token()].store(1, Ordering::SeqCst);
                })
            },
            {
                let stage0 = Arc::clone(&stage0);
                let stage1 = Arc::clone(&stage1);
                Pipe::parallel(move |ctx| {
                    let value = stage0[ctx.token()].load(Ordering::SeqCst);
                    stage1[ctx.token()].store(value + 1, Ordering::SeqCst);
                })
            },
            {
                let stage1 = Arc::clone(&stage1);
                let stage2 = Arc::clone(&stage2);
                Pipe::serial(move |ctx| {
                    let value = stage1[ctx.token()].load(Ordering::SeqCst);
                    stage2[ctx.token()].store(value + 1, Ordering::SeqCst);
                })
            },
        ],
    );

    pipeline
        .run(&executor)
        .wait()
        .expect("pipeline run should succeed");

    assert_eq!(pipeline.num_tokens(), 5);
    for token in 0..5 {
        assert_eq!(stage0[token].load(Ordering::SeqCst), 1);
        assert_eq!(stage1[token].load(Ordering::SeqCst), 2);
        assert_eq!(stage2[token].load(Ordering::SeqCst), 3);
    }
    assert_eq!(stage0[5].load(Ordering::SeqCst), 0);
    assert_eq!(stage1[5].load(Ordering::SeqCst), 0);
    assert_eq!(stage2[5].load(Ordering::SeqCst), 0);
}

#[test]
fn serial_and_parallel_pipes_apply_expected_concurrency() {
    let executor = Executor::new(4);
    let active_parallel = Arc::new(AtomicUsize::new(0));
    let max_parallel = Arc::new(AtomicUsize::new(0));
    let active_serial = Arc::new(AtomicUsize::new(0));
    let max_serial = Arc::new(AtomicUsize::new(0));

    let pipeline = Pipeline::from_pipes(
        4,
        vec![
            Pipe::serial(|ctx| {
                if ctx.token() == 8 {
                    ctx.stop();
                }
            }),
            {
                let active_parallel = Arc::clone(&active_parallel);
                let max_parallel = Arc::clone(&max_parallel);
                Pipe::parallel(move |_| {
                    let current = active_parallel.fetch_add(1, Ordering::SeqCst) + 1;
                    max_parallel.fetch_max(current, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                    active_parallel.fetch_sub(1, Ordering::SeqCst);
                })
            },
            {
                let active_serial = Arc::clone(&active_serial);
                let max_serial = Arc::clone(&max_serial);
                Pipe::serial(move |_| {
                    let current = active_serial.fetch_add(1, Ordering::SeqCst) + 1;
                    max_serial.fetch_max(current, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(2));
                    active_serial.fetch_sub(1, Ordering::SeqCst);
                })
            },
        ],
    );

    pipeline
        .run(&executor)
        .wait()
        .expect("pipeline run should succeed");

    assert_eq!(pipeline.num_tokens(), 8);
    assert_eq!(max_serial.load(Ordering::SeqCst), 1);
    assert!(
        max_parallel.load(Ordering::SeqCst) > 1,
        "parallel pipe should overlap across lines"
    );
}

#[test]
fn stop_token_is_only_valid_on_the_first_pipe() {
    let executor = Executor::new(2);
    let pipeline = Pipeline::from_pipes(
        2,
        vec![
            Pipe::serial(|ctx| {
                if ctx.token() == 1 {
                    ctx.stop();
                }
            }),
            Pipe::parallel(|ctx| {
                ctx.stop();
            }),
        ],
    );

    let error = pipeline
        .run(&executor)
        .wait()
        .expect_err("non-first pipe stop should fail");

    assert!(error.message().contains("only the first pipe"));
}

#[test]
fn pipeline_can_be_reset_and_reused() {
    let executor = Executor::new(2);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let pipeline = Pipeline::from_pipes(
        2,
        vec![{
            let seen = Arc::clone(&seen);
            Pipe::serial(move |ctx| {
                if ctx.token() == 3 {
                    ctx.stop();
                    return;
                }
                seen.lock().expect("seen poisoned").push(ctx.token());
            })
        }],
    );

    pipeline
        .run(&executor)
        .wait()
        .expect("first pipeline run should succeed");
    assert_eq!(pipeline.num_tokens(), 3);

    pipeline.reset();
    pipeline
        .run(&executor)
        .wait()
        .expect("second pipeline run should succeed");
    assert_eq!(pipeline.num_tokens(), 3);

    let seen = seen.lock().expect("seen poisoned");
    assert_eq!(seen.as_slice(), &[0, 1, 2, 0, 1, 2]);
}
