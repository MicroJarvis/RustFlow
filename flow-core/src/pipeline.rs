use std::any::Any;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use crate::Executor;
use crate::error::FlowError;
use crate::executor::RunHandle;

type DataPayload = Box<dyn Any + Send>;
type DataPayloadSlot = Option<DataPayload>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipeType {
    Serial,
    Parallel,
}

#[derive(Clone)]
pub struct Pipe {
    pipe_type: PipeType,
    task: Arc<dyn Fn(&mut PipeContext) + Send + Sync + 'static>,
}

impl Pipe {
    pub fn new<F>(pipe_type: PipeType, task: F) -> Self
    where
        F: Fn(&mut PipeContext) + Send + Sync + 'static,
    {
        Self {
            pipe_type,
            task: Arc::new(task),
        }
    }

    pub fn serial<F>(task: F) -> Self
    where
        F: Fn(&mut PipeContext) + Send + Sync + 'static,
    {
        Self::new(PipeType::Serial, task)
    }

    pub fn parallel<F>(task: F) -> Self
    where
        F: Fn(&mut PipeContext) + Send + Sync + 'static,
    {
        Self::new(PipeType::Parallel, task)
    }

    pub fn pipe_type(&self) -> PipeType {
        self.pipe_type
    }

    fn run(&self, context: &mut PipeContext) {
        (self.task)(context);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipeContext {
    line: usize,
    pipe: usize,
    token: usize,
    stop_requested: bool,
}

impl PipeContext {
    pub fn line(&self) -> usize {
        self.line
    }

    pub fn pipe(&self) -> usize {
        self.pipe
    }

    pub fn token(&self) -> usize {
        self.token
    }

    pub fn stop(&mut self) {
        assert!(
            self.pipe == 0,
            "only the first pipe can request a pipeline stop"
        );
        self.stop_requested = true;
    }
}

struct StaticPipelineInner {
    num_lines: usize,
    pipes: Vec<Pipe>,
    num_tokens: Mutex<usize>,
    run_lock: Mutex<()>,
}

#[derive(Clone)]
pub struct Pipeline {
    inner: Arc<StaticPipelineInner>,
}

impl Pipeline {
    pub fn from_pipes(num_lines: usize, pipes: Vec<Pipe>) -> Self {
        validate_pipe_config(num_lines, &pipes);

        Self {
            inner: Arc::new(StaticPipelineInner {
                num_lines,
                pipes,
                num_tokens: Mutex::new(0),
                run_lock: Mutex::new(()),
            }),
        }
    }

    pub fn num_lines(&self) -> usize {
        self.inner.num_lines
    }

    pub fn num_pipes(&self) -> usize {
        self.inner.pipes.len()
    }

    pub fn num_tokens(&self) -> usize {
        *lock_unpoisoned(&self.inner.num_tokens)
    }

    pub fn reset(&self) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        *lock_unpoisoned(&self.inner.num_tokens) = 0;
    }

    pub fn run(&self, executor: &Executor) -> RunHandle {
        let pipeline = self.clone();
        let executor = executor.clone();
        spawn_run_handle(move |cancelled| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            *lock_unpoisoned(&pipeline.inner.num_tokens) = 0;
            run_pipe_chain(
                &executor,
                pipeline.inner.num_lines,
                pipeline.inner.pipes.clone(),
                &pipeline.inner.num_tokens,
                cancelled,
            )
        })
    }
}

#[derive(Clone)]
struct ScalablePipelineConfig {
    num_lines: usize,
    pipes: Vec<Pipe>,
}

struct ScalablePipelineInner {
    config: Mutex<ScalablePipelineConfig>,
    num_tokens: Mutex<usize>,
    run_lock: Mutex<()>,
}

#[derive(Clone)]
pub struct ScalablePipeline {
    inner: Arc<ScalablePipelineInner>,
}

impl ScalablePipeline {
    pub fn new(num_lines: usize) -> Self {
        assert!(num_lines > 0, "pipeline must have at least one line");

        Self {
            inner: Arc::new(ScalablePipelineInner {
                config: Mutex::new(ScalablePipelineConfig {
                    num_lines,
                    pipes: Vec::new(),
                }),
                num_tokens: Mutex::new(0),
                run_lock: Mutex::new(()),
            }),
        }
    }

    pub fn from_pipes(num_lines: usize, pipes: Vec<Pipe>) -> Self {
        validate_resettable_pipe_config(num_lines, &pipes);

        Self {
            inner: Arc::new(ScalablePipelineInner {
                config: Mutex::new(ScalablePipelineConfig { num_lines, pipes }),
                num_tokens: Mutex::new(0),
                run_lock: Mutex::new(()),
            }),
        }
    }

    pub fn num_lines(&self) -> usize {
        lock_unpoisoned(&self.inner.config).num_lines
    }

    pub fn num_pipes(&self) -> usize {
        lock_unpoisoned(&self.inner.config).pipes.len()
    }

    pub fn num_tokens(&self) -> usize {
        *lock_unpoisoned(&self.inner.num_tokens)
    }

    pub fn reset(&self) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        *lock_unpoisoned(&self.inner.num_tokens) = 0;
    }

    pub fn reset_with_pipes(&self, pipes: Vec<Pipe>) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        let mut config = lock_unpoisoned(&self.inner.config);
        validate_resettable_pipe_config(config.num_lines, &pipes);
        config.pipes = pipes;
        *lock_unpoisoned(&self.inner.num_tokens) = 0;
    }

    pub fn reset_with_lines_and_pipes(&self, num_lines: usize, pipes: Vec<Pipe>) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        validate_resettable_pipe_config(num_lines, &pipes);
        *lock_unpoisoned(&self.inner.config) = ScalablePipelineConfig { num_lines, pipes };
        *lock_unpoisoned(&self.inner.num_tokens) = 0;
    }

    pub fn run(&self, executor: &Executor) -> RunHandle {
        let pipeline = self.clone();
        let executor = executor.clone();
        spawn_run_handle(move |cancelled| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            *lock_unpoisoned(&pipeline.inner.num_tokens) = 0;
            let config = lock_unpoisoned(&pipeline.inner.config).clone();
            if config.pipes.is_empty() {
                return Err(FlowError::plain("pipeline has no pipes configured"));
            }

            run_pipe_chain(
                &executor,
                config.num_lines,
                config.pipes,
                &pipeline.inner.num_tokens,
                cancelled,
            )
        })
    }
}

#[derive(Clone)]
struct DataStage {
    pipe_type: PipeType,
    task: Arc<dyn Fn(DataPayloadSlot, &mut PipeContext) -> DataPayloadSlot + Send + Sync + 'static>,
}

impl DataStage {
    fn source<O, F>(pipe_type: PipeType, task: F) -> Self
    where
        O: Send + 'static,
        F: Fn(&mut PipeContext) -> O + Send + Sync + 'static,
    {
        Self {
            pipe_type,
            task: Arc::new(move |_input, context| Some(Box::new(task(context)))),
        }
    }

    fn stage<I, O, F>(pipe_type: PipeType, task: F) -> Self
    where
        I: Send + 'static,
        O: Send + 'static,
        F: Fn(&mut I, &mut PipeContext) -> O + Send + Sync + 'static,
    {
        Self {
            pipe_type,
            task: Arc::new(move |input, context| {
                let input = input.expect("data pipeline stage expected an input payload");
                let mut input = *input
                    .downcast::<I>()
                    .expect("data pipeline stage input type mismatch");
                Some(Box::new(task(&mut input, context)))
            }),
        }
    }

    fn sink<I, F>(pipe_type: PipeType, task: F) -> Self
    where
        I: Send + 'static,
        F: Fn(&mut I, &mut PipeContext) + Send + Sync + 'static,
    {
        Self {
            pipe_type,
            task: Arc::new(move |input, context| {
                let input = input.expect("data pipeline sink expected an input payload");
                let mut input = *input
                    .downcast::<I>()
                    .expect("data pipeline sink input type mismatch");
                task(&mut input, context);
                None
            }),
        }
    }

    fn pipe_type(&self) -> PipeType {
        self.pipe_type
    }

    fn run(&self, input: DataPayloadSlot, context: &mut PipeContext) -> DataPayloadSlot {
        (self.task)(input, context)
    }
}

struct DataPipelineInner {
    num_lines: usize,
    stages: Vec<DataStage>,
    num_tokens: Mutex<usize>,
    run_lock: Mutex<()>,
}

#[derive(Clone)]
pub struct DataPipeline {
    inner: Arc<DataPipelineInner>,
}

impl DataPipeline {
    pub fn builder(num_lines: usize) -> DataPipelineBuilder<()> {
        assert!(num_lines > 0, "pipeline must have at least one line");
        DataPipelineBuilder {
            num_lines,
            stages: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn num_lines(&self) -> usize {
        self.inner.num_lines
    }

    pub fn num_pipes(&self) -> usize {
        self.inner.stages.len()
    }

    pub fn num_tokens(&self) -> usize {
        *lock_unpoisoned(&self.inner.num_tokens)
    }

    pub fn reset(&self) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        *lock_unpoisoned(&self.inner.num_tokens) = 0;
    }

    pub fn run(&self, executor: &Executor) -> RunHandle {
        let pipeline = self.clone();
        let executor = executor.clone();
        spawn_run_handle(move |cancelled| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            *lock_unpoisoned(&pipeline.inner.num_tokens) = 0;

            run_data_chain(
                &executor,
                pipeline.inner.num_lines,
                pipeline.inner.stages.clone(),
                &pipeline.inner.num_tokens,
                cancelled,
            )
        })
    }

    fn from_stages(num_lines: usize, stages: Vec<DataStage>) -> Self {
        validate_data_stage_config(num_lines, &stages);

        Self {
            inner: Arc::new(DataPipelineInner {
                num_lines,
                stages,
                num_tokens: Mutex::new(0),
                run_lock: Mutex::new(()),
            }),
        }
    }
}

pub struct DataPipelineBuilder<I> {
    num_lines: usize,
    stages: Vec<DataStage>,
    _marker: PhantomData<fn() -> I>,
}

impl DataPipelineBuilder<()> {
    pub fn source<O, F>(mut self, pipe_type: PipeType, task: F) -> DataPipelineBuilder<O>
    where
        O: Send + 'static,
        F: Fn(&mut PipeContext) -> O + Send + Sync + 'static,
    {
        self.stages.push(DataStage::source(pipe_type, task));
        DataPipelineBuilder {
            num_lines: self.num_lines,
            stages: self.stages,
            _marker: PhantomData,
        }
    }
}

impl<I> DataPipelineBuilder<I>
where
    I: Send + 'static,
{
    pub fn stage<O, F>(mut self, pipe_type: PipeType, task: F) -> DataPipelineBuilder<O>
    where
        O: Send + 'static,
        F: Fn(&mut I, &mut PipeContext) -> O + Send + Sync + 'static,
    {
        self.stages.push(DataStage::stage(pipe_type, task));
        DataPipelineBuilder {
            num_lines: self.num_lines,
            stages: self.stages,
            _marker: PhantomData,
        }
    }

    pub fn sink<F>(mut self, pipe_type: PipeType, task: F) -> DataPipeline
    where
        F: Fn(&mut I, &mut PipeContext) + Send + Sync + 'static,
    {
        self.stages.push(DataStage::sink(pipe_type, task));
        DataPipeline::from_stages(self.num_lines, self.stages)
    }

    pub fn build(self) -> DataPipeline {
        DataPipeline::from_stages(self.num_lines, self.stages)
    }
}

struct PipelineRunState {
    next_token: Mutex<usize>,
    stopped: AtomicBool,
    cancelled: Arc<AtomicBool>,
    stage_locks: Vec<Option<Mutex<()>>>,
}

impl PipelineRunState {
    fn new(stage_locks: Vec<Option<Mutex<()>>>, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            next_token: Mutex::new(0),
            stopped: AtomicBool::new(false),
            cancelled,
            stage_locks,
        }
    }

    fn request_stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.cancelled.store(true, Ordering::Release);
    }

    fn should_stop(&self) -> bool {
        self.stopped.load(Ordering::Acquire) || self.cancelled.load(Ordering::Acquire)
    }
}

fn run_pipe_chain(
    executor: &Executor,
    num_lines: usize,
    pipes: Vec<Pipe>,
    num_tokens: &Mutex<usize>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), FlowError> {
    let pipes: Arc<[Pipe]> = Arc::from(pipes);
    let run_state = Arc::new(PipelineRunState::new(
        pipes
            .iter()
            .map(|pipe| (pipe.pipe_type() == PipeType::Serial).then_some(Mutex::new(())))
            .collect(),
        cancelled,
    ));

    let handles = (0..num_lines)
        .map(|line| {
            let pipes = Arc::clone(&pipes);
            let run_state = Arc::clone(&run_state);
            executor.async_task(move || run_pipe_line(pipes, line, run_state))
        })
        .collect::<Vec<_>>();

    let result = wait_pipeline_handles(handles, &run_state);
    *lock_unpoisoned(num_tokens) = *lock_unpoisoned(&run_state.next_token);
    result
}

fn run_pipe_line(
    pipes: Arc<[Pipe]>,
    line: usize,
    run_state: Arc<PipelineRunState>,
) -> Result<(), FlowError> {
    loop {
        if run_state.should_stop() {
            return Ok(());
        }

        let mut context = PipeContext {
            line,
            pipe: 0,
            token: 0,
            stop_requested: false,
        };

        {
            let _stage_guard = lock_unpoisoned(
                run_state.stage_locks[0]
                    .as_ref()
                    .expect("the first pipe must be serial"),
            );

            if run_state.should_stop() {
                return Ok(());
            }

            let mut next_token = lock_unpoisoned(&run_state.next_token);
            context.token = *next_token;
            pipes[0].run(&mut context);

            if context.stop_requested {
                run_state.request_stop();
                return Ok(());
            }

            *next_token += 1;
        }

        // Once a line has claimed a token from stage 0, let it drain through the
        // remaining stages before honoring a global stop or cancellation request.
        for pipe_index in 1..pipes.len() {
            context.pipe = pipe_index;
            if let Some(stage_lock) = &run_state.stage_locks[pipe_index] {
                let _stage_guard = lock_unpoisoned(stage_lock);
                pipes[pipe_index].run(&mut context);
            } else {
                pipes[pipe_index].run(&mut context);
            }
        }
    }
}

fn run_data_chain(
    executor: &Executor,
    num_lines: usize,
    stages: Vec<DataStage>,
    num_tokens: &Mutex<usize>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), FlowError> {
    let stages: Arc<[DataStage]> = Arc::from(stages);
    let run_state = Arc::new(PipelineRunState::new(
        stages
            .iter()
            .map(|stage| (stage.pipe_type() == PipeType::Serial).then_some(Mutex::new(())))
            .collect(),
        cancelled,
    ));

    let handles = (0..num_lines)
        .map(|line| {
            let stages = Arc::clone(&stages);
            let run_state = Arc::clone(&run_state);
            executor.async_task(move || run_data_line(stages, line, run_state))
        })
        .collect::<Vec<_>>();

    let result = wait_pipeline_handles(handles, &run_state);
    *lock_unpoisoned(num_tokens) = *lock_unpoisoned(&run_state.next_token);
    result
}

fn run_data_line(
    stages: Arc<[DataStage]>,
    line: usize,
    run_state: Arc<PipelineRunState>,
) -> Result<(), FlowError> {
    loop {
        if run_state.should_stop() {
            return Ok(());
        }

        let mut context = PipeContext {
            line,
            pipe: 0,
            token: 0,
            stop_requested: false,
        };
        let mut payload = {
            let _stage_guard = lock_unpoisoned(
                run_state.stage_locks[0]
                    .as_ref()
                    .expect("the first pipe must be serial"),
            );

            if run_state.should_stop() {
                return Ok(());
            }

            let mut next_token = lock_unpoisoned(&run_state.next_token);
            context.token = *next_token;
            let payload = stages[0].run(None, &mut context);

            if context.stop_requested {
                run_state.request_stop();
                return Ok(());
            }

            *next_token += 1;
            payload
        };

        // Once a line has claimed a token from stage 0, let it drain through the
        // remaining stages before honoring a global stop or cancellation request.
        for pipe_index in 1..stages.len() {
            context.pipe = pipe_index;
            if let Some(stage_lock) = &run_state.stage_locks[pipe_index] {
                let _stage_guard = lock_unpoisoned(stage_lock);
                payload = stages[pipe_index].run(payload, &mut context);
            } else {
                payload = stages[pipe_index].run(payload, &mut context);
            }
        }
    }
}

fn wait_pipeline_handles<T>(
    handles: Vec<crate::AsyncHandle<Result<T, FlowError>>>,
    run_state: &Arc<PipelineRunState>,
) -> Result<T, FlowError> {
    let mut first_error = None;
    let mut last_value = None;

    for handle in handles {
        match handle.wait() {
            Ok(Ok(value)) => last_value = Some(value),
            Ok(Err(error)) | Err(error) => {
                run_state.request_stop();
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(last_value.expect("pipeline worker tasks should produce a completion marker"))
    }
}

fn spawn_run_handle<F>(run: F) -> RunHandle
where
    F: FnOnce(Arc<AtomicBool>) -> Result<(), FlowError> + Send + 'static,
{
    let handle = RunHandle::pending();
    let cancelled = handle.cancelled_flag();
    let handle_clone = handle.clone();

    thread::spawn(move || {
        let result = run(cancelled);
        handle_clone.complete(result);
    });

    handle
}

fn validate_pipe_config(num_lines: usize, pipes: &[Pipe]) {
    assert!(num_lines > 0, "pipeline must have at least one line");
    assert!(!pipes.is_empty(), "pipeline must have at least one pipe");
    assert!(
        pipes[0].pipe_type() == PipeType::Serial,
        "the first pipe must be serial"
    );
}

fn validate_resettable_pipe_config(num_lines: usize, pipes: &[Pipe]) {
    assert!(num_lines > 0, "pipeline must have at least one line");
    if pipes.is_empty() {
        return;
    }
    assert!(
        pipes[0].pipe_type() == PipeType::Serial,
        "the first pipe must be serial"
    );
}

fn validate_data_stage_config(num_lines: usize, stages: &[DataStage]) {
    assert!(num_lines > 0, "pipeline must have at least one line");
    assert!(
        !stages.is_empty(),
        "data pipeline must have at least one stage"
    );
    assert!(
        stages[0].pipe_type() == PipeType::Serial,
        "the first pipe must be serial"
    );
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}
