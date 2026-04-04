use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::Executor;
use crate::error::FlowError;
use crate::executor::RunHandle;
use crate::runtime::RuntimeCtx;

struct LineSlot<T> {
    values: Vec<UnsafeCell<MaybeUninit<T>>>,
    initialized: Vec<AtomicBool>,
}

unsafe impl<T: Send> Send for LineSlot<T> {}
unsafe impl<T: Send> Sync for LineSlot<T> {}

impl<T> LineSlot<T> {
    fn new(num_lines: usize) -> Self {
        Self {
            values: (0..num_lines)
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
            initialized: (0..num_lines).map(|_| AtomicBool::new(false)).collect(),
        }
    }

    fn write(&self, line: usize, value: T) {
        if self.initialized[line].swap(true, Ordering::AcqRel) {
            panic!("data pipeline line slot written before previous value was consumed");
        }

        unsafe {
            (*self.values[line].get()).write(value);
        }
    }

    fn take(&self, line: usize) -> T {
        if !self.initialized[line].swap(false, Ordering::AcqRel) {
            panic!("data pipeline line slot should be initialized before use");
        }

        unsafe { (*self.values[line].get()).assume_init_read() }
    }
}

impl<T> Drop for LineSlot<T> {
    fn drop(&mut self) {
        for line in 0..self.values.len() {
            if self.initialized[line].load(Ordering::Acquire) {
                unsafe {
                    self.values[line].get_mut().assume_init_drop();
                }
            }
        }
    }
}

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
        executor.schedule_runtime_runner(Box::new(move |runtime| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            *lock_unpoisoned(&pipeline.inner.num_tokens) = 0;
            run_pipe_chain(
                runtime,
                pipeline.inner.num_lines,
                pipeline.inner.pipes.clone(),
                &pipeline.inner.num_tokens,
            )
        }))
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
        executor.schedule_runtime_runner(Box::new(move |runtime| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            *lock_unpoisoned(&pipeline.inner.num_tokens) = 0;
            let config = lock_unpoisoned(&pipeline.inner.config).clone();
            if config.pipes.is_empty() {
                return Err(FlowError::plain("pipeline has no pipes configured"));
            }

            run_pipe_chain(
                runtime,
                config.num_lines,
                config.pipes,
                &pipeline.inner.num_tokens,
            )
        }))
    }
}

#[derive(Clone)]
struct DataStage {
    runner: Arc<dyn DataStageRunner>,
}

trait DataStageRunner: Send + Sync {
    fn pipe_type(&self) -> PipeType;
    fn run(&self, line: usize, context: &mut PipeContext);
}

struct SourceDataStage<O, F> {
    pipe_type: PipeType,
    task: F,
    output: Arc<LineSlot<O>>,
}

impl<O, F> DataStageRunner for SourceDataStage<O, F>
where
    O: Send + 'static,
    F: Fn(&mut PipeContext) -> O + Send + Sync + 'static,
{
    fn pipe_type(&self) -> PipeType {
        self.pipe_type
    }

    fn run(&self, line: usize, context: &mut PipeContext) {
        self.output.write(line, (self.task)(context));
    }
}

struct TransformDataStage<I, O, F> {
    pipe_type: PipeType,
    task: F,
    input: Arc<LineSlot<I>>,
    output: Arc<LineSlot<O>>,
}

impl<I, O, F> DataStageRunner for TransformDataStage<I, O, F>
where
    I: Send + 'static,
    O: Send + 'static,
    F: Fn(&mut I, &mut PipeContext) -> O + Send + Sync + 'static,
{
    fn pipe_type(&self) -> PipeType {
        self.pipe_type
    }

    fn run(&self, line: usize, context: &mut PipeContext) {
        let mut input = self.input.take(line);
        self.output.write(line, (self.task)(&mut input, context));
    }
}

struct SinkDataStage<I, F> {
    pipe_type: PipeType,
    task: F,
    input: Arc<LineSlot<I>>,
}

impl<I, F> DataStageRunner for SinkDataStage<I, F>
where
    I: Send + 'static,
    F: Fn(&mut I, &mut PipeContext) + Send + Sync + 'static,
{
    fn pipe_type(&self) -> PipeType {
        self.pipe_type
    }

    fn run(&self, line: usize, context: &mut PipeContext) {
        let mut input = self.input.take(line);
        (self.task)(&mut input, context);
    }
}

impl DataStage {
    fn source<O, F>(num_lines: usize, pipe_type: PipeType, task: F) -> (Self, Arc<LineSlot<O>>)
    where
        O: Send + 'static,
        F: Fn(&mut PipeContext) -> O + Send + Sync + 'static,
    {
        let output = Arc::new(LineSlot::new(num_lines));
        (
            Self {
                runner: Arc::new(SourceDataStage {
                    pipe_type,
                    task,
                    output: Arc::clone(&output),
                }),
            },
            output,
        )
    }

    fn stage<I, O, F>(
        num_lines: usize,
        pipe_type: PipeType,
        input: Arc<LineSlot<I>>,
        task: F,
    ) -> (Self, Arc<LineSlot<O>>)
    where
        I: Send + 'static,
        O: Send + 'static,
        F: Fn(&mut I, &mut PipeContext) -> O + Send + Sync + 'static,
    {
        let output = Arc::new(LineSlot::new(num_lines));
        (
            Self {
                runner: Arc::new(TransformDataStage {
                    pipe_type,
                    task,
                    input,
                    output: Arc::clone(&output),
                }),
            },
            output,
        )
    }

    fn sink<I, F>(pipe_type: PipeType, input: Arc<LineSlot<I>>, task: F) -> Self
    where
        I: Send + 'static,
        F: Fn(&mut I, &mut PipeContext) + Send + Sync + 'static,
    {
        Self {
            runner: Arc::new(SinkDataStage {
                pipe_type,
                task,
                input,
            }),
        }
    }

    fn pipe_type(&self) -> PipeType {
        self.runner.pipe_type()
    }

    fn run(&self, line: usize, context: &mut PipeContext) {
        self.runner.run(line, context);
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
            output_slot: None,
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
        executor.schedule_runtime_runner(Box::new(move |runtime| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            *lock_unpoisoned(&pipeline.inner.num_tokens) = 0;

            run_data_chain(
                runtime,
                pipeline.inner.num_lines,
                pipeline.inner.stages.clone(),
                &pipeline.inner.num_tokens,
            )
        }))
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
    output_slot: Option<Arc<LineSlot<I>>>,
    _marker: PhantomData<fn() -> I>,
}

impl DataPipelineBuilder<()> {
    pub fn source<O, F>(mut self, pipe_type: PipeType, task: F) -> DataPipelineBuilder<O>
    where
        O: Send + 'static,
        F: Fn(&mut PipeContext) -> O + Send + Sync + 'static,
    {
        let (stage, output_slot) = DataStage::source(self.num_lines, pipe_type, task);
        self.stages.push(stage);
        DataPipelineBuilder {
            num_lines: self.num_lines,
            stages: self.stages,
            output_slot: Some(output_slot),
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
        let input = self
            .output_slot
            .take()
            .expect("data pipeline stage expected a preceding output slot");
        let (stage, output_slot) = DataStage::stage(self.num_lines, pipe_type, input, task);
        self.stages.push(stage);
        DataPipelineBuilder {
            num_lines: self.num_lines,
            stages: self.stages,
            output_slot: Some(output_slot),
            _marker: PhantomData,
        }
    }

    pub fn sink<F>(mut self, pipe_type: PipeType, task: F) -> DataPipeline
    where
        F: Fn(&mut I, &mut PipeContext) + Send + Sync + 'static,
    {
        let input = self
            .output_slot
            .take()
            .expect("data pipeline sink expected a preceding output slot");
        self.stages.push(DataStage::sink(pipe_type, input, task));
        DataPipeline::from_stages(self.num_lines, self.stages)
    }

    pub fn build(self) -> DataPipeline {
        DataPipeline::from_stages(self.num_lines, self.stages)
    }
}

struct PipelineRunState {
    next_token: AtomicUsize,
    stopped: AtomicBool,
    cancelled: Arc<AtomicBool>,
    stage_locks: Vec<Option<Mutex<()>>>,
}

impl PipelineRunState {
    fn new(stage_locks: Vec<Option<Mutex<()>>>, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            next_token: AtomicUsize::new(0),
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
    runtime: &RuntimeCtx,
    num_lines: usize,
    pipes: Vec<Pipe>,
    num_tokens: &Mutex<usize>,
) -> Result<(), FlowError> {
    let pipes: Arc<[Pipe]> = Arc::from(pipes);
    let run_state = Arc::new(PipelineRunState::new(
        pipes
            .iter()
            .map(|pipe| (pipe.pipe_type() == PipeType::Serial).then_some(Mutex::new(())))
            .collect(),
        runtime.cancelled_flag(),
    ));

    for line in 1..num_lines {
        let pipes = Arc::clone(&pipes);
        let run_state = Arc::clone(&run_state);
        runtime.silent_result_async(move |_| {
            let result = run_pipe_line(pipes, line, Arc::clone(&run_state));
            if result.is_err() {
                run_state.request_stop();
            }
            result
        });
    }

    let inline_result = run_pipe_line(Arc::clone(&pipes), 0, Arc::clone(&run_state));
    if inline_result.is_err() {
        run_state.request_stop();
    }

    let children_result = runtime.corun_children();
    let result = match inline_result {
        Ok(()) => children_result,
        Err(error) => {
            let _ = children_result;
            Err(error)
        }
    };
    *lock_unpoisoned(num_tokens) = run_state.next_token.load(Ordering::Acquire);
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

            let next_token = run_state.next_token.load(Ordering::Acquire);
            context.token = next_token;
            pipes[0].run(&mut context);

            if context.stop_requested {
                run_state.request_stop();
                return Ok(());
            }

            run_state
                .next_token
                .store(next_token + 1, Ordering::Release);
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
    runtime: &RuntimeCtx,
    num_lines: usize,
    stages: Vec<DataStage>,
    num_tokens: &Mutex<usize>,
) -> Result<(), FlowError> {
    let stages: Arc<[DataStage]> = Arc::from(stages);
    let run_state = Arc::new(PipelineRunState::new(
        stages
            .iter()
            .map(|stage| (stage.pipe_type() == PipeType::Serial).then_some(Mutex::new(())))
            .collect(),
        runtime.cancelled_flag(),
    ));

    for line in 1..num_lines {
        let stages = Arc::clone(&stages);
        let run_state = Arc::clone(&run_state);
        runtime.silent_result_async(move |_| {
            let result = run_data_line(stages, line, Arc::clone(&run_state));
            if result.is_err() {
                run_state.request_stop();
            }
            result
        });
    }

    let inline_result = run_data_line(Arc::clone(&stages), 0, Arc::clone(&run_state));
    if inline_result.is_err() {
        run_state.request_stop();
    }

    let children_result = runtime.corun_children();
    let result = match inline_result {
        Ok(()) => children_result,
        Err(error) => {
            let _ = children_result;
            Err(error)
        }
    };
    *lock_unpoisoned(num_tokens) = run_state.next_token.load(Ordering::Acquire);
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
        {
            let _stage_guard = lock_unpoisoned(
                run_state.stage_locks[0]
                    .as_ref()
                    .expect("the first pipe must be serial"),
            );

            if run_state.should_stop() {
                return Ok(());
            }

            let next_token = run_state.next_token.load(Ordering::Acquire);
            context.token = next_token;
            stages[0].run(line, &mut context);

            if context.stop_requested {
                run_state.request_stop();
                return Ok(());
            }

            run_state
                .next_token
                .store(next_token + 1, Ordering::Release);
        }

        // Once a line has claimed a token from stage 0, let it drain through the
        // remaining stages before honoring a global stop or cancellation request.
        for pipe_index in 1..stages.len() {
            context.pipe = pipe_index;
            if let Some(stage_lock) = &run_state.stage_locks[pipe_index] {
                let _stage_guard = lock_unpoisoned(stage_lock);
                stages[pipe_index].run(line, &mut context);
            } else {
                stages[pipe_index].run(line, &mut context);
            }
        }
    }
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
