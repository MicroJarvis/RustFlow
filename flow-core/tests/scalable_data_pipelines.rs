use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flow_core::{DataPipeline, Executor, Pipe, PipeContext, PipeType, ScalablePipeline};

#[test]
fn scalable_pipeline_can_reset_pipe_ranges_and_restart_tokens() {
    let executor = Executor::new(4);
    let first_run = Arc::new(Mutex::new(Vec::new()));
    let second_run = Arc::new(Mutex::new(Vec::new()));
    let pipeline = ScalablePipeline::new(2);

    pipeline.reset_with_pipes(vec![
        {
            let first_run = Arc::clone(&first_run);
            Pipe::serial(move |ctx: &mut PipeContext| {
                if ctx.token() == 3 {
                    ctx.stop();
                    return;
                }
                first_run
                    .lock()
                    .expect("first_run poisoned")
                    .push((ctx.pipe(), ctx.token()));
            })
        },
        {
            let first_run = Arc::clone(&first_run);
            Pipe::parallel(move |ctx: &mut PipeContext| {
                first_run
                    .lock()
                    .expect("first_run poisoned")
                    .push((ctx.pipe(), ctx.token()));
            })
        },
    ]);

    pipeline
        .run(&executor)
        .wait()
        .expect("first scalable pipeline run should succeed");

    assert_eq!(pipeline.num_lines(), 2);
    assert_eq!(pipeline.num_pipes(), 2);
    assert_eq!(pipeline.num_tokens(), 3);
    assert_eq!(first_run.lock().expect("first_run poisoned").len(), 6);

    pipeline.reset_with_lines_and_pipes(
        3,
        vec![
            {
                let second_run = Arc::clone(&second_run);
                Pipe::serial(move |ctx: &mut PipeContext| {
                    if ctx.token() == 3 {
                        ctx.stop();
                        return;
                    }
                    second_run
                        .lock()
                        .expect("second_run poisoned")
                        .push((ctx.pipe(), ctx.token()));
                })
            },
            {
                let second_run = Arc::clone(&second_run);
                Pipe::parallel(move |ctx: &mut PipeContext| {
                    second_run
                        .lock()
                        .expect("second_run poisoned")
                        .push((ctx.pipe(), ctx.token()));
                })
            },
            {
                let second_run = Arc::clone(&second_run);
                Pipe::serial(move |ctx: &mut PipeContext| {
                    second_run
                        .lock()
                        .expect("second_run poisoned")
                        .push((ctx.pipe(), ctx.token()));
                })
            },
        ],
    );

    pipeline
        .run(&executor)
        .wait()
        .expect("second scalable pipeline run should succeed");

    assert_eq!(pipeline.num_lines(), 3);
    assert_eq!(pipeline.num_pipes(), 3);
    assert_eq!(pipeline.num_tokens(), 3);
    assert_eq!(second_run.lock().expect("second_run poisoned").len(), 9);
}

#[test]
fn data_pipeline_moves_values_between_stages() {
    let executor = Executor::new(4);
    let outputs = Arc::new((0..6).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    let pipeline = DataPipeline::builder(4)
        .source(PipeType::Serial, |ctx| -> usize {
            if ctx.token() == 5 {
                ctx.stop();
                return 0;
            }
            ctx.token()
        })
        .stage(PipeType::Parallel, |value: &mut usize, _ctx| -> usize {
            *value += 10;
            *value
        })
        .sink(PipeType::Serial, {
            let outputs = Arc::clone(&outputs);
            move |value: &mut usize, ctx| {
                outputs[ctx.token()].store(*value, Ordering::SeqCst);
            }
        });

    pipeline
        .run(&executor)
        .wait()
        .expect("data pipeline run should succeed");

    assert_eq!(pipeline.num_tokens(), 5);
    for token in 0..5 {
        assert_eq!(outputs[token].load(Ordering::SeqCst), token + 10);
    }
    assert_eq!(outputs[5].load(Ordering::SeqCst), 0);
}

#[test]
fn data_pipeline_supports_heterogeneous_stage_storage() {
    #[derive(Clone)]
    struct LargePayload([usize; 8]);

    let executor = Executor::new(4);
    let outputs = Arc::new((0..5).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());

    let pipeline = DataPipeline::builder(2)
        .source(PipeType::Serial, |ctx| -> usize {
            if ctx.token() == 4 {
                ctx.stop();
                return 0;
            }
            ctx.token()
        })
        .stage(PipeType::Parallel, |value: &mut usize, _ctx| -> String {
            (*value + 1).to_string()
        })
        .stage(
            PipeType::Parallel,
            |value: &mut String, _ctx| -> LargePayload {
                LargePayload([value.parse::<usize>().expect("value should be numeric"); 8])
            },
        )
        .stage(
            PipeType::Parallel,
            |value: &mut LargePayload, _ctx| -> usize { value.0[0] },
        )
        .sink(PipeType::Serial, {
            let outputs = Arc::clone(&outputs);
            move |value: &mut usize, ctx| {
                outputs[ctx.token()].store(*value, Ordering::SeqCst);
            }
        });

    pipeline
        .run(&executor)
        .wait()
        .expect("heterogeneous data pipeline run should succeed");

    assert_eq!(pipeline.num_tokens(), 4);
    for token in 0..4 {
        assert_eq!(outputs[token].load(Ordering::SeqCst), token + 1);
    }
    assert_eq!(outputs[4].load(Ordering::SeqCst), 0);
}
