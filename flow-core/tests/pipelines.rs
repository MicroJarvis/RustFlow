use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flow_core::{Executor, Pipe, Pipeline, PipelineProfile};

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
fn all_serial_pipeline_counts_each_stage_once_per_token() {
    let executor = Executor::new(4);
    let stage_counts = Arc::new((0..8).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let stop_seen = Arc::new(AtomicUsize::new(0));
    let unexpected_token_eight = Arc::new(AtomicUsize::new(0));
    let final_stage_order = Arc::new(Mutex::new(Vec::new()));

    let mut pipes = Vec::new();
    for pipe_index in 0..8 {
        let stage_counts = Arc::clone(&stage_counts);
        let stop_seen = Arc::clone(&stop_seen);
        let unexpected_token_eight = Arc::clone(&unexpected_token_eight);
        let final_stage_order = Arc::clone(&final_stage_order);
        pipes.push(Pipe::serial(move |ctx| {
            if ctx.token() == 8 {
                if pipe_index == 0 {
                    stop_seen.fetch_add(1, Ordering::SeqCst);
                    ctx.stop();
                } else {
                    unexpected_token_eight.fetch_add(1, Ordering::SeqCst);
                }
                return;
            }

            stage_counts[pipe_index].fetch_add(1, Ordering::SeqCst);
            if pipe_index == 7 {
                final_stage_order
                    .lock()
                    .expect("final_stage_order poisoned")
                    .push(ctx.token());
            }
        }));
    }

    let pipeline = Pipeline::from_pipes(4, pipes);
    pipeline
        .run(&executor)
        .wait()
        .expect("all-serial pipeline run should succeed");

    assert_eq!(pipeline.num_tokens(), 8);
    assert_eq!(stop_seen.load(Ordering::SeqCst), 1);
    assert_eq!(unexpected_token_eight.load(Ordering::SeqCst), 0);
    for pipe_index in 0..8 {
        assert_eq!(stage_counts[pipe_index].load(Ordering::SeqCst), 8);
    }
    assert_eq!(
        final_stage_order
            .lock()
            .expect("final_stage_order poisoned")
            .as_slice(),
        &[0, 1, 2, 3, 4, 5, 6, 7]
    );
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

#[test]
fn pipeline_profile_records_serial_lock_and_corun_wait_activity() {
    let executor = Executor::new(4);
    let pipeline = Pipeline::from_pipes(
        4,
        vec![
            Pipe::serial(|ctx| {
                if ctx.token() == 12 {
                    ctx.stop();
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }),
            Pipe::serial(|_| {
                std::thread::sleep(Duration::from_millis(1));
            }),
            Pipe::serial(|_| {
                std::thread::sleep(Duration::from_millis(1));
            }),
        ],
    );

    pipeline.set_profile_enabled(true);
    pipeline
        .run(&executor)
        .wait()
        .expect("profiled pipeline run should succeed");

    let profile = pipeline.last_profile();
    assert!(!profile.is_empty(), "profile should capture some activity");
    assert!(
        profile.stage0_contended_acquires > 0
            || profile.downstream_contended_acquires > 0
            || profile.corun_idle_spins > 0
            || profile.corun_inline_executions > 0,
        "profile should record either serial-stage contention or corun wait activity: {profile}"
    );
    assert!(
        profile.corun_total_wait_ns > 0
            || profile.stage0_contended_acquires > 0
            || profile.downstream_contended_acquires > 0,
        "profile should capture either corun wait or serial-stage coordination: {profile}"
    );

    pipeline.set_profile_enabled(false);
    pipeline
        .run(&executor)
        .wait()
        .expect("non-profiled pipeline run should still succeed");
    assert_eq!(pipeline.last_profile(), PipelineProfile::default());
}
