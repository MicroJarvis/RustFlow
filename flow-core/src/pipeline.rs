use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// A lightweight spinlock for short-held critical sections.
/// More efficient than Mutex for very short lock durations.
struct SpinLock {
    locked: AtomicBool,
}

// SAFETY: SpinLock uses AtomicBool which is inherently thread-safe.
// The lock can be safely shared between threads.
unsafe impl Send for SpinLock {}
unsafe impl Sync for SpinLock {}

impl SpinLock {
    fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    fn try_lock(&self) -> Option<SpinLockGuard<'_>> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| SpinLockGuard {
                locked: &self.locked,
            })
    }
}

struct SpinLockGuard<'a> {
    locked: &'a AtomicBool,
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.locked.store(false, Ordering::Release);
    }
}

use crate::Executor;
use crate::error::FlowError;
use crate::executor::RunHandle;
use crate::perf::{PipelineProfile, PipelineProfileState};
use crate::runtime::RuntimeCtx;

struct LineSlot<T> {
    values: Vec<UnsafeCell<MaybeUninit<T>>>,
    initialized: Vec<UnsafeCell<bool>>,
}

// SAFETY: Each pipeline line owns a distinct slot index for the full run, so line-slot
// payloads and initialized flags are only touched by one worker per index at a time.
unsafe impl<T: Send> Send for LineSlot<T> {}
unsafe impl<T: Send> Sync for LineSlot<T> {}

impl<T> LineSlot<T> {
    fn new(num_lines: usize) -> Self {
        Self {
            values: (0..num_lines)
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
            initialized: (0..num_lines).map(|_| UnsafeCell::new(false)).collect(),
        }
    }

    fn write(&self, line: usize, value: T) {
        unsafe {
            let initialized = &mut *self.initialized[line].get();
            if *initialized {
                panic!("data pipeline line slot written before previous value was consumed");
            }

            (*self.values[line].get()).write(value);
            *initialized = true;
        }
    }

    fn take(&self, line: usize) -> T {
        unsafe {
            let initialized = &mut *self.initialized[line].get();
            if !*initialized {
                panic!("data pipeline line slot should be initialized before use");
            }

            *initialized = false;
            (*self.values[line].get()).assume_init_read()
        }
    }
}

impl<T> Drop for LineSlot<T> {
    fn drop(&mut self) {
        for line in 0..self.values.len() {
            if *self.initialized[line].get_mut() {
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
    num_tokens: AtomicUsize,
    profile_enabled: AtomicBool,
    last_profile: Mutex<PipelineProfile>,
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
                num_tokens: AtomicUsize::new(0),
                profile_enabled: AtomicBool::new(false),
                last_profile: Mutex::new(PipelineProfile::default()),
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
        self.inner.num_tokens.load(Ordering::Acquire)
    }

    pub fn set_profile_enabled(&self, enabled: bool) {
        self.inner.profile_enabled.store(enabled, Ordering::Release);
        if !enabled {
            *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
        }
    }

    pub fn last_profile(&self) -> PipelineProfile {
        *lock_unpoisoned(&self.inner.last_profile)
    }

    pub fn reset(&self) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        self.inner.num_tokens.store(0, Ordering::Release);
        *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
    }

    pub fn run(&self, executor: &Executor) -> RunHandle {
        let pipeline = self.clone();
        executor.schedule_runtime_runner(Box::new(move |runtime| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            pipeline.inner.num_tokens.store(0, Ordering::Release);
            let profile_state = pipeline
                .inner
                .profile_enabled
                .load(Ordering::Acquire)
                .then(|| Arc::new(PipelineProfileState::default()));
            let result = run_pipe_chain(
                runtime,
                pipeline.inner.num_lines,
                pipeline.inner.pipes.clone(),
                &pipeline.inner.num_tokens,
                profile_state.clone(),
            );
            *lock_unpoisoned(&pipeline.inner.last_profile) = profile_state
                .as_deref()
                .map_or_else(PipelineProfile::default, PipelineProfileState::snapshot);
            result
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
    num_tokens: AtomicUsize,
    profile_enabled: AtomicBool,
    last_profile: Mutex<PipelineProfile>,
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
                num_tokens: AtomicUsize::new(0),
                profile_enabled: AtomicBool::new(false),
                last_profile: Mutex::new(PipelineProfile::default()),
                run_lock: Mutex::new(()),
            }),
        }
    }

    pub fn from_pipes(num_lines: usize, pipes: Vec<Pipe>) -> Self {
        validate_resettable_pipe_config(num_lines, &pipes);

        Self {
            inner: Arc::new(ScalablePipelineInner {
                config: Mutex::new(ScalablePipelineConfig { num_lines, pipes }),
                num_tokens: AtomicUsize::new(0),
                profile_enabled: AtomicBool::new(false),
                last_profile: Mutex::new(PipelineProfile::default()),
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
        self.inner.num_tokens.load(Ordering::Acquire)
    }

    pub fn set_profile_enabled(&self, enabled: bool) {
        self.inner.profile_enabled.store(enabled, Ordering::Release);
        if !enabled {
            *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
        }
    }

    pub fn last_profile(&self) -> PipelineProfile {
        *lock_unpoisoned(&self.inner.last_profile)
    }

    pub fn reset(&self) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        self.inner.num_tokens.store(0, Ordering::Release);
        *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
    }

    pub fn reset_with_pipes(&self, pipes: Vec<Pipe>) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        let mut config = lock_unpoisoned(&self.inner.config);
        validate_resettable_pipe_config(config.num_lines, &pipes);
        config.pipes = pipes;
        self.inner.num_tokens.store(0, Ordering::Release);
        *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
    }

    pub fn reset_with_lines_and_pipes(&self, num_lines: usize, pipes: Vec<Pipe>) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        validate_resettable_pipe_config(num_lines, &pipes);
        *lock_unpoisoned(&self.inner.config) = ScalablePipelineConfig { num_lines, pipes };
        self.inner.num_tokens.store(0, Ordering::Release);
        *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
    }

    pub fn run(&self, executor: &Executor) -> RunHandle {
        let pipeline = self.clone();
        executor.schedule_runtime_runner(Box::new(move |runtime| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            pipeline.inner.num_tokens.store(0, Ordering::Release);
            let config = lock_unpoisoned(&pipeline.inner.config).clone();
            if config.pipes.is_empty() {
                return Err(FlowError::plain("pipeline has no pipes configured"));
            }

            let profile_state = pipeline
                .inner
                .profile_enabled
                .load(Ordering::Acquire)
                .then(|| Arc::new(PipelineProfileState::default()));
            let result = run_pipe_chain(
                runtime,
                config.num_lines,
                config.pipes,
                &pipeline.inner.num_tokens,
                profile_state.clone(),
            );
            *lock_unpoisoned(&pipeline.inner.last_profile) = profile_state
                .as_deref()
                .map_or_else(PipelineProfile::default, PipelineProfileState::snapshot);
            result
        }))
    }
}

struct DataStage {
    runner: Box<dyn DataStageRunner>,
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
                runner: Box::new(SourceDataStage {
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
                runner: Box::new(TransformDataStage {
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
            runner: Box::new(SinkDataStage {
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
    stages: Arc<[DataStage]>,
    num_tokens: AtomicUsize,
    profile_enabled: AtomicBool,
    last_profile: Mutex<PipelineProfile>,
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
        self.inner.num_tokens.load(Ordering::Acquire)
    }

    pub fn set_profile_enabled(&self, enabled: bool) {
        self.inner.profile_enabled.store(enabled, Ordering::Release);
        if !enabled {
            *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
        }
    }

    pub fn last_profile(&self) -> PipelineProfile {
        *lock_unpoisoned(&self.inner.last_profile)
    }

    pub fn reset(&self) {
        let _run_guard = lock_unpoisoned(&self.inner.run_lock);
        self.inner.num_tokens.store(0, Ordering::Release);
        *lock_unpoisoned(&self.inner.last_profile) = PipelineProfile::default();
    }

    pub fn run(&self, executor: &Executor) -> RunHandle {
        let pipeline = self.clone();
        executor.schedule_runtime_runner(Box::new(move |runtime| {
            let _run_guard = lock_unpoisoned(&pipeline.inner.run_lock);
            pipeline.inner.num_tokens.store(0, Ordering::Release);

            let profile_state = pipeline
                .inner
                .profile_enabled
                .load(Ordering::Acquire)
                .then(|| Arc::new(PipelineProfileState::default()));
            let result = run_data_chain(
                runtime,
                pipeline.inner.num_lines,
                Arc::clone(&pipeline.inner.stages),
                &pipeline.inner.num_tokens,
                profile_state.clone(),
            );
            *lock_unpoisoned(&pipeline.inner.last_profile) = profile_state
                .as_deref()
                .map_or_else(PipelineProfile::default, PipelineProfileState::snapshot);
            result
        }))
    }

    fn from_stages(num_lines: usize, stages: Vec<DataStage>) -> Self {
        validate_data_stage_config(num_lines, &stages);

        Self {
            inner: Arc::new(DataPipelineInner {
                num_lines,
                stages: Arc::from(stages),
                num_tokens: AtomicUsize::new(0),
                profile_enabled: AtomicBool::new(false),
                last_profile: Mutex::new(PipelineProfile::default()),
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
    flags: AtomicU8,
    cancelled: Arc<AtomicBool>,
    stage_locks: Vec<Option<SpinLock>>,
    profile: Option<Arc<PipelineProfileState>>,
}

impl PipelineRunState {
    fn new(
        stage_is_serial: Vec<bool>,
        cancelled: Arc<AtomicBool>,
        profile: Option<Arc<PipelineProfileState>>,
    ) -> Self {
        Self {
            next_token: AtomicUsize::new(0),
            flags: AtomicU8::new(0),
            cancelled,
            stage_locks: stage_is_serial
                .into_iter()
                .map(|is_serial| is_serial.then_some(SpinLock::new()))
                .collect(),
            profile,
        }
    }

    fn request_stop(&self) {
        self.flags.fetch_or(Self::STOP_REQUESTED, Ordering::Release);
    }

    fn request_abort(&self) {
        self.flags
            .fetch_or(Self::ABORT_REQUESTED, Ordering::Release);
    }

    fn should_stop_issuing_tokens(&self) -> bool {
        self.flags.load(Ordering::Acquire) != 0 || self.cancelled.load(Ordering::Acquire)
    }

    fn should_abort_immediately(&self) -> bool {
        self.flags.load(Ordering::Acquire) & Self::ABORT_REQUESTED != 0
            || self.cancelled.load(Ordering::Acquire)
    }

    const STOP_REQUESTED: u8 = 0b01;
    const ABORT_REQUESTED: u8 = 0b10;
}

enum SerialStageAcquire<'a> {
    Acquired(SpinLockGuard<'a>),
    StopRequested,
}

fn acquire_serial_stage<'a>(
    stage_lock: &'a SpinLock,
    run_state: &PipelineRunState,
    stop_if_requested: bool,
) -> SerialStageAcquire<'a> {
    const STOP_CHECK_INTERVAL: usize = 8;
    if let Some(guard) = stage_lock.try_lock() {
        if let Some(profile) = run_state.profile.as_deref() {
            profile.record_stage_fast_path(stop_if_requested);
        }
        return SerialStageAcquire::Acquired(guard);
    }

    let wait_started = run_state.profile.as_ref().map(|_| Instant::now());
    let max_spins = if stop_if_requested { 32 } else { 16 };
    let mut retry_checks = 0usize;
    let mut spins_since_yield = 0usize;
    let mut total_spins = 0u64;
    let mut total_yields = 0u64;

    loop {
        retry_checks += 1;
        if let Some(guard) = stage_lock.try_lock() {
            if let Some(profile) = run_state.profile.as_deref() {
                profile.record_stage_wait(
                    stop_if_requested,
                    wait_started
                        .as_ref()
                        .expect("profiled stage wait should have a start time")
                        .elapsed(),
                    total_spins,
                    total_yields,
                );
            }
            return SerialStageAcquire::Acquired(guard);
        }

        if retry_checks % STOP_CHECK_INTERVAL == 0
            && (run_state.should_abort_immediately()
                || (stop_if_requested && run_state.should_stop_issuing_tokens()))
        {
            if let Some(profile) = run_state.profile.as_deref() {
                profile.record_stage_wait(
                    stop_if_requested,
                    wait_started
                        .as_ref()
                        .expect("profiled stage wait should have a start time")
                        .elapsed(),
                    total_spins,
                    total_yields,
                );
            }
            return SerialStageAcquire::StopRequested;
        }

        spins_since_yield += 1;
        if spins_since_yield < max_spins {
            total_spins += 1;
            std::hint::spin_loop();
            continue;
        }

        spins_since_yield = 0;
        total_yields += 1;
        std::thread::yield_now();
    }
}
fn run_pipe_chain(
    runtime: &RuntimeCtx,
    num_lines: usize,
    pipes: Vec<Pipe>,
    num_tokens: &AtomicUsize,
    profile: Option<Arc<PipelineProfileState>>,
) -> Result<(), FlowError> {
    let pipes: Arc<[Pipe]> = Arc::from(pipes);
    let stage_is_serial: Vec<bool> = pipes
        .iter()
        .map(|pipe| pipe.pipe_type() == PipeType::Serial)
        .collect();
    let spawn_lines_globally = should_global_spawn_pipeline_lines(&stage_is_serial);
    let run_state = Arc::new(PipelineRunState::new(
        stage_is_serial,
        runtime.cancelled_flag(),
        profile,
    ));
    let batch_size = compute_batch_size(num_lines);

    for line in 1..num_lines {
        let pipes = Arc::clone(&pipes);
        let run_state = Arc::clone(&run_state);
        if spawn_lines_globally {
            runtime.silent_result_async_global(move |_| {
                let result = run_pipe_line(pipes, line, Arc::clone(&run_state), batch_size);
                if result.is_err() {
                    run_state.request_abort();
                }
                result
            });
        } else {
            runtime.silent_result_async(move |_| {
                let result = run_pipe_line(pipes, line, Arc::clone(&run_state), batch_size);
                if result.is_err() {
                    run_state.request_abort();
                }
                result
            });
        }
    }

    let inline_result = run_pipe_line(Arc::clone(&pipes), 0, Arc::clone(&run_state), batch_size);
    if inline_result.is_err() {
        run_state.request_abort();
    }

    let children_result = runtime.corun_children_profiled(run_state.profile.as_deref());
    let result = match inline_result {
        Ok(()) => children_result,
        Err(error) => {
            let _ = children_result;
            Err(error)
        }
    };
    num_tokens.store(
        run_state.next_token.load(Ordering::Acquire),
        Ordering::Release,
    );
    result
}

/// Compute an adaptive batch size based on the number of pipeline lines.
/// For single-line pipelines, batch aggressively since there's no inter-line contention.
/// For multi-line, batch_size=1 preserves strict token ordering across stages.
fn compute_batch_size(num_lines: usize) -> usize {
    if num_lines <= 1 { 16 } else { 1 }
}

fn should_global_spawn_pipeline_lines(stage_is_serial: &[bool]) -> bool {
    // All-serial pipelines rely on parent-local line scheduling to preserve
    // deterministic token order at serial sinks. Mixed pipelines still benefit
    // from making line runners immediately stealable by other workers.
    stage_is_serial.iter().any(|is_serial| !*is_serial)
}

fn run_pipe_line(
    pipes: Arc<[Pipe]>,
    line: usize,
    run_state: Arc<PipelineRunState>,
    batch_size: usize,
) -> Result<(), FlowError> {
    loop {
        if run_state.should_stop_issuing_tokens() {
            return Ok(());
        }

        // Acquire stage 0 lock once and issue a batch of tokens.
        let batch_start;
        let mut batch_count = 0usize;
        {
            let stage0_lock = run_state.stage_locks[0]
                .as_ref()
                .expect("the first pipe must be serial");

            let SerialStageAcquire::Acquired(_stage_guard) =
                acquire_serial_stage(stage0_lock, &run_state, true)
            else {
                return Ok(());
            };

            if run_state.should_stop_issuing_tokens() {
                return Ok(());
            }

            batch_start = run_state.next_token.load(Ordering::Acquire);

            for i in 0..batch_size {
                let token = batch_start + i;
                let mut context = PipeContext {
                    line,
                    pipe: 0,
                    token,
                    stop_requested: false,
                };
                pipes[0].run(&mut context);
                if context.stop_requested {
                    // Don't count the stop token; commit only successfully issued tokens.
                    run_state
                        .next_token
                        .store(batch_start + batch_count, Ordering::Release);
                    run_state.request_stop();
                    // Still process remaining stages for already-issued tokens below.
                    break;
                }
                batch_count = i + 1;
            }

            if !run_state.should_stop_issuing_tokens() {
                run_state
                    .next_token
                    .store(batch_start + batch_count, Ordering::Release);
            }
        }

        // Process each issued token through all remaining stages, maintaining
        // token ordering (same as original per-token processing).
        for i in 0..batch_count {
            let token = batch_start + i;
            let mut context = PipeContext {
                line,
                pipe: 0,
                token,
                stop_requested: false,
            };
            for pipe_index in 1..pipes.len() {
                context.pipe = pipe_index;
                if let Some(stage_lock) = &run_state.stage_locks[pipe_index] {
                    let SerialStageAcquire::Acquired(_stage_guard) =
                        acquire_serial_stage(stage_lock, &run_state, false)
                    else {
                        return Ok(());
                    };
                    pipes[pipe_index].run(&mut context);
                } else {
                    pipes[pipe_index].run(&mut context);
                }
            }
        }

        if run_state.should_stop_issuing_tokens() {
            return Ok(());
        }
    }
}

fn run_data_chain(
    runtime: &RuntimeCtx,
    num_lines: usize,
    stages: Arc<[DataStage]>,
    num_tokens: &AtomicUsize,
    profile: Option<Arc<PipelineProfileState>>,
) -> Result<(), FlowError> {
    let stage_is_serial: Vec<bool> = stages
        .iter()
        .map(|stage| stage.pipe_type() == PipeType::Serial)
        .collect();
    let spawn_lines_globally = should_global_spawn_pipeline_lines(&stage_is_serial);
    let run_state = Arc::new(PipelineRunState::new(
        stage_is_serial,
        runtime.cancelled_flag(),
        profile,
    ));
    let batch_size = compute_batch_size(num_lines);

    for line in 1..num_lines {
        let stages = Arc::clone(&stages);
        let run_state = Arc::clone(&run_state);
        if spawn_lines_globally {
            runtime.silent_result_async_global(move |_| {
                let result = run_data_line(stages, line, Arc::clone(&run_state), batch_size);
                if result.is_err() {
                    run_state.request_abort();
                }
                result
            });
        } else {
            runtime.silent_result_async(move |_| {
                let result = run_data_line(stages, line, Arc::clone(&run_state), batch_size);
                if result.is_err() {
                    run_state.request_abort();
                }
                result
            });
        }
    }

    let inline_result = run_data_line(Arc::clone(&stages), 0, Arc::clone(&run_state), batch_size);
    if inline_result.is_err() {
        run_state.request_abort();
    }

    let children_result = runtime.corun_children_profiled(run_state.profile.as_deref());
    let result = match inline_result {
        Ok(()) => children_result,
        Err(error) => {
            let _ = children_result;
            Err(error)
        }
    };
    num_tokens.store(
        run_state.next_token.load(Ordering::Acquire),
        Ordering::Release,
    );
    result
}

fn run_data_line(
    stages: Arc<[DataStage]>,
    line: usize,
    run_state: Arc<PipelineRunState>,
    batch_size: usize,
) -> Result<(), FlowError> {
    loop {
        if run_state.should_stop_issuing_tokens() {
            return Ok(());
        }

        let batch_start;
        let mut batch_count = 0usize;
        {
            let stage0_lock = run_state.stage_locks[0]
                .as_ref()
                .expect("the first pipe must be serial");

            let SerialStageAcquire::Acquired(_stage_guard) =
                acquire_serial_stage(stage0_lock, &run_state, true)
            else {
                return Ok(());
            };

            if run_state.should_stop_issuing_tokens() {
                return Ok(());
            }

            batch_start = run_state.next_token.load(Ordering::Acquire);

            for i in 0..batch_size {
                let token = batch_start + i;
                let mut context = PipeContext {
                    line,
                    pipe: 0,
                    token,
                    stop_requested: false,
                };
                stages[0].run(line, &mut context);
                if context.stop_requested {
                    run_state
                        .next_token
                        .store(batch_start + batch_count, Ordering::Release);
                    run_state.request_stop();
                    break;
                }
                batch_count = i + 1;
            }

            if !run_state.should_stop_issuing_tokens() {
                run_state
                    .next_token
                    .store(batch_start + batch_count, Ordering::Release);
            }
        }

        for i in 0..batch_count {
            let token = batch_start + i;
            let mut context = PipeContext {
                line,
                pipe: 0,
                token,
                stop_requested: false,
            };
            for pipe_index in 1..stages.len() {
                context.pipe = pipe_index;
                if let Some(stage_lock) = &run_state.stage_locks[pipe_index] {
                    let SerialStageAcquire::Acquired(_stage_guard) =
                        acquire_serial_stage(stage_lock, &run_state, false)
                    else {
                        return Ok(());
                    };
                    stages[pipe_index].run(line, &mut context);
                } else {
                    stages[pipe_index].run(line, &mut context);
                }
            }
        }

        if run_state.should_stop_issuing_tokens() {
            return Ok(());
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
