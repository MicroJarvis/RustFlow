use std::any::Any;
use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Instant;

use flow_algorithms::{
    Executor, ParallelForOptions, parallel_find, parallel_inclusive_scan, parallel_reduce,
    parallel_sort, parallel_transform,
};
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};

const FIND_NEEDLE: u64 = 0x9e37_79b9_7f4a_7c15;
const CHECKSUM_OFFSET: u64 = 1_469_598_103_934_665_603;
const CHECKSUM_PRIME: u64 = 1_099_511_628_211;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BackendId {
    Flow,
    ThreadPool,
    Rayon,
    CppTaskflow,
}

impl BackendId {
    pub fn all() -> Vec<Self> {
        vec![Self::Flow, Self::ThreadPool, Self::Rayon, Self::CppTaskflow]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::ThreadPool => "threadpool",
            Self::Rayon => "rayon",
            Self::CppTaskflow => "cpp-taskflow",
        }
    }

    fn parse_list(value: &str) -> Result<Vec<Self>, BenchmarkError> {
        value
            .split(',')
            .map(|token| match token.trim() {
                "flow" => Ok(Self::Flow),
                "threadpool" => Ok(Self::ThreadPool),
                "rayon" => Ok(Self::Rayon),
                "cpp-taskflow" => Ok(Self::CppTaskflow),
                other => Err(BenchmarkError::new(format!("unknown backend `{other}`"))),
            })
            .collect()
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScenarioId {
    Transform,
    Reduce,
    Find,
    InclusiveScan,
    Sort,
}

impl ScenarioId {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Transform,
            Self::Reduce,
            Self::Find,
            Self::InclusiveScan,
            Self::Sort,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transform => "transform",
            Self::Reduce => "reduce",
            Self::Find => "find",
            Self::InclusiveScan => "inclusive-scan",
            Self::Sort => "sort",
        }
    }

    fn parse_list(value: &str) -> Result<Vec<Self>, BenchmarkError> {
        value
            .split(',')
            .map(|token| match token.trim() {
                "transform" => Ok(Self::Transform),
                "reduce" => Ok(Self::Reduce),
                "find" => Ok(Self::Find),
                "inclusive-scan" => Ok(Self::InclusiveScan),
                "sort" => Ok(Self::Sort),
                other => Err(BenchmarkError::new(format!("unknown scenario `{other}`"))),
            })
            .collect()
    }
}

impl fmt::Display for ScenarioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).as_str())
    }
}

#[derive(Clone, Debug)]
pub struct BenchmarkConfig {
    pub backends: Vec<BackendId>,
    pub scenarios: Vec<ScenarioId>,
    pub input_len: usize,
    pub worker_count: usize,
    pub chunk_size: usize,
    pub warmup_iterations: usize,
    pub measure_iterations: usize,
    pub seed: u64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            backends: BackendId::all(),
            scenarios: ScenarioId::all(),
            input_len: 65_536,
            worker_count: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(4)
                .max(1),
            chunk_size: 1_024,
            warmup_iterations: 1,
            measure_iterations: 5,
            seed: 42,
        }
    }
}

impl BenchmarkConfig {
    pub fn for_test() -> Self {
        Self::default()
            .with_input_len(256)
            .with_worker_count(4)
            .with_chunk_size(32)
            .with_warmup_iterations(0)
            .with_measure_iterations(1)
    }

    pub fn with_backends(mut self, backends: Vec<BackendId>) -> Self {
        self.backends = backends;
        self
    }

    pub fn with_scenarios(mut self, scenarios: Vec<ScenarioId>) -> Self {
        self.scenarios = scenarios;
        self
    }

    pub fn with_input_len(mut self, input_len: usize) -> Self {
        self.input_len = input_len;
        self
    }

    pub fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = worker_count.max(1);
        self
    }

    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.max(1);
        self
    }

    pub fn with_warmup_iterations(mut self, warmup_iterations: usize) -> Self {
        self.warmup_iterations = warmup_iterations;
        self
    }

    pub fn with_measure_iterations(mut self, measure_iterations: usize) -> Self {
        self.measure_iterations = measure_iterations.max(1);
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

#[derive(Clone, Debug)]
pub struct BenchmarkEntry {
    pub backend: BackendId,
    pub scenario: ScenarioId,
    pub available: bool,
    pub reason: Option<String>,
    pub samples_ns: Vec<u128>,
    pub min_ns: u128,
    pub max_ns: u128,
    pub median_ns: u128,
    pub mean_ns: u128,
    pub checksum: u64,
}

#[derive(Clone, Debug)]
pub struct BenchmarkReport {
    pub config: BenchmarkConfig,
    pub entries: Vec<BenchmarkEntry>,
}

impl BenchmarkReport {
    pub fn statuses(&self) -> &[BenchmarkEntry] {
        &self.entries
    }

    pub fn to_json(&self) -> String {
        let backends = self
            .config
            .backends
            .iter()
            .map(|backend| format!("\"{}\"", backend.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let scenarios = self
            .config
            .scenarios
            .iter()
            .map(|scenario| format!("\"{}\"", scenario.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                let samples = entry
                    .samples_ns
                    .iter()
                    .map(u128::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let reason = entry
                    .reason
                    .as_ref()
                    .map(|reason| format!("\"{}\"", json_escape(reason)))
                    .unwrap_or_else(|| "null".to_string());
                format!(
                    "{{\"backend\":\"{}\",\"scenario\":\"{}\",\"available\":{},\"reason\":{},\"samples_ns\":[{}],\"min_ns\":{},\"max_ns\":{},\"median_ns\":{},\"mean_ns\":{},\"checksum\":{}}}",
                    entry.backend.as_str(),
                    entry.scenario.as_str(),
                    entry.available,
                    reason,
                    samples,
                    entry.min_ns,
                    entry.max_ns,
                    entry.median_ns,
                    entry.mean_ns,
                    entry.checksum
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"config\":{{\"input_len\":{},\"worker_count\":{},\"chunk_size\":{},\"warmup_iterations\":{},\"measure_iterations\":{},\"seed\":{},\"backends\":[{}],\"scenarios\":[{}]}},\"entries\":[{}]}}",
            self.config.input_len,
            self.config.worker_count,
            self.config.chunk_size,
            self.config.warmup_iterations,
            self.config.measure_iterations,
            self.config.seed,
            backends,
            scenarios,
            entries
        )
    }

    pub fn to_table(&self) -> String {
        let mut lines = Vec::with_capacity(self.entries.len() + 2);
        lines.push(
            "backend       scenario         status       median(ns)   min(ns)      max(ns)      mean(ns)     checksum"
                .to_string(),
        );
        lines.push(
            "------------  ---------------  -----------  -----------  -----------  -----------  -----------  -----------"
                .to_string(),
        );
        for entry in &self.entries {
            let status = if entry.available { "ok" } else { "unavailable" };
            let detail = if entry.available {
                format!(
                    "{:<12}  {:<15}  {:<11}  {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
                    entry.backend.as_str(),
                    entry.scenario.as_str(),
                    status,
                    entry.median_ns,
                    entry.min_ns,
                    entry.max_ns,
                    entry.mean_ns,
                    entry.checksum
                )
            } else {
                let reason = entry.reason.as_deref().unwrap_or("unknown");
                format!(
                    "{:<12}  {:<15}  {:<11}  {}",
                    entry.backend.as_str(),
                    entry.scenario.as_str(),
                    status,
                    reason
                )
            };
            lines.push(detail);
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkError {
    message: String,
}

impl BenchmarkError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for BenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BenchmarkError {}

struct Dataset {
    values: Arc<[u64]>,
    needle: u64,
}

impl Dataset {
    fn new(len: usize, seed: u64) -> Self {
        let mut state = seed | 1;
        let mut values = Vec::with_capacity(len);

        for _ in 0..len {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mut value = state ^ (state >> 33);
            if value == FIND_NEEDLE {
                value ^= 0xa076_1d64_78bd_642f;
            }
            values.push(value);
        }

        if let Some(slot) = values.get_mut(len.saturating_mul(3) / 4) {
            *slot = FIND_NEEDLE;
        }

        Self {
            values: values.into(),
            needle: FIND_NEEDLE,
        }
    }

    fn sort_input(&self) -> Vec<u64> {
        self.values.to_vec()
    }
}

pub fn run_benchmarks(config: BenchmarkConfig) -> Result<BenchmarkReport, BenchmarkError> {
    let dataset = Dataset::new(config.input_len, config.seed);
    let flow = FlowBackend::new(&config);
    let threadpool = ThreadPoolBackend::new(&config);
    let rayon = RayonBackend::new(&config)?;
    let cpp = CppTaskflowBackend::new();

    let mut entries = Vec::with_capacity(config.backends.len() * config.scenarios.len());

    for backend in &config.backends {
        for scenario in &config.scenarios {
            let entry = match backend {
                BackendId::Flow => available_entry(
                    *backend,
                    *scenario,
                    flow.measure(*scenario, &dataset, &config)?,
                ),
                BackendId::ThreadPool => available_entry(
                    *backend,
                    *scenario,
                    threadpool.measure(*scenario, &dataset, &config)?,
                ),
                BackendId::Rayon => available_entry(
                    *backend,
                    *scenario,
                    rayon.measure(*scenario, &dataset, &config)?,
                ),
                BackendId::CppTaskflow => match cpp.measure(*scenario, &config) {
                    Ok(measurement) => available_entry(*backend, *scenario, measurement),
                    Err(reason) => unavailable_entry(*backend, *scenario, reason),
                },
            };
            entries.push(entry);
        }
    }

    Ok(BenchmarkReport { config, entries })
}

pub fn run_from_args<I>(args: I) -> Result<(BenchmarkReport, OutputFormat), BenchmarkError>
where
    I: IntoIterator<Item = String>,
{
    let mut config = BenchmarkConfig::default();
    let mut format = OutputFormat::Table;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backends" => {
                let value = next_arg(&mut args, "--backends")?;
                config = config.with_backends(BackendId::parse_list(&value)?);
            }
            "--scenarios" => {
                let value = next_arg(&mut args, "--scenarios")?;
                config = config.with_scenarios(ScenarioId::parse_list(&value)?);
            }
            "--input-len" => {
                config = config.with_input_len(parse_usize(&mut args, "--input-len")?);
            }
            "--workers" => {
                config = config.with_worker_count(parse_usize(&mut args, "--workers")?);
            }
            "--chunk-size" => {
                config = config.with_chunk_size(parse_usize(&mut args, "--chunk-size")?);
            }
            "--warmups" => {
                config = config.with_warmup_iterations(parse_usize(&mut args, "--warmups")?);
            }
            "--iterations" => {
                config = config.with_measure_iterations(parse_usize(&mut args, "--iterations")?);
            }
            "--seed" => {
                config = config.with_seed(parse_u64(&mut args, "--seed")?);
            }
            "--format" => {
                let value = next_arg(&mut args, "--format")?;
                format = match value.as_str() {
                    "json" => OutputFormat::Json,
                    "table" => OutputFormat::Table,
                    other => {
                        return Err(BenchmarkError::new(format!(
                            "unknown output format `{other}`"
                        )));
                    }
                };
            }
            "--help" | "-h" => {
                return Err(BenchmarkError::new(help_text()));
            }
            other => {
                return Err(BenchmarkError::new(format!("unknown argument `{other}`")));
            }
        }
    }

    Ok((run_benchmarks(config)?, format))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Json,
    Table,
}

struct Measurement {
    samples_ns: Vec<u128>,
    checksum: u64,
}

struct FlowBackend {
    executor: Executor,
    options: ParallelForOptions,
}

impl FlowBackend {
    fn new(config: &BenchmarkConfig) -> Self {
        Self {
            executor: Executor::new(config.worker_count),
            options: ParallelForOptions::default().with_chunk_size(config.chunk_size),
        }
    }

    fn measure(
        &self,
        scenario: ScenarioId,
        dataset: &Dataset,
        config: &BenchmarkConfig,
    ) -> Result<Measurement, BenchmarkError> {
        measure_iterations(config, || self.run_once(scenario, dataset))
    }

    fn run_once(&self, scenario: ScenarioId, dataset: &Dataset) -> Result<u64, BenchmarkError> {
        match scenario {
            ScenarioId::Transform => {
                let output = parallel_transform(
                    &self.executor,
                    Arc::clone(&dataset.values),
                    self.options,
                    |value| transform_value(*value),
                )
                .map_err(|error| BenchmarkError::new(error.to_string()))?;
                Ok(checksum_slice(&output))
            }
            ScenarioId::Reduce => parallel_reduce(
                &self.executor,
                Arc::clone(&dataset.values),
                self.options,
                0u64,
                |left, right| left.wrapping_add(right),
            )
            .map_err(|error| BenchmarkError::new(error.to_string())),
            ScenarioId::Find => {
                parallel_find(&self.executor, Arc::clone(&dataset.values), self.options, {
                    let needle = dataset.needle;
                    move |value| *value == needle
                })
                .map(|index| index.map_or(u64::MAX, |value| value as u64))
                .map_err(|error| BenchmarkError::new(error.to_string()))
            }
            ScenarioId::InclusiveScan => {
                let output = parallel_inclusive_scan(
                    &self.executor,
                    Arc::clone(&dataset.values),
                    self.options,
                    |left, right| left.wrapping_add(right),
                )
                .map_err(|error| BenchmarkError::new(error.to_string()))?;
                Ok(checksum_slice(&output))
            }
            ScenarioId::Sort => {
                let output = parallel_sort(&self.executor, dataset.sort_input(), self.options)
                    .map_err(|error| BenchmarkError::new(error.to_string()))?;
                Ok(checksum_slice(&output))
            }
        }
    }
}

struct ThreadPoolBackend {
    pool: SimpleThreadPool,
    chunk_size: usize,
}

impl ThreadPoolBackend {
    fn new(config: &BenchmarkConfig) -> Self {
        Self {
            pool: SimpleThreadPool::new(config.worker_count),
            chunk_size: config.chunk_size,
        }
    }

    fn measure(
        &self,
        scenario: ScenarioId,
        dataset: &Dataset,
        config: &BenchmarkConfig,
    ) -> Result<Measurement, BenchmarkError> {
        measure_iterations(config, || self.run_once(scenario, dataset))
    }

    fn run_once(&self, scenario: ScenarioId, dataset: &Dataset) -> Result<u64, BenchmarkError> {
        match scenario {
            ScenarioId::Transform => {
                let ranges = chunk_ranges(dataset.values.len(), self.chunk_size);
                let handles = ranges
                    .into_iter()
                    .map(|range| {
                        let values = Arc::clone(&dataset.values);
                        self.pool.submit(move || {
                            range
                                .map(|index| transform_value(values[index]))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(checksum_slice(
                    &wait_all(handles)?.into_iter().flatten().collect::<Vec<_>>(),
                ))
            }
            ScenarioId::Reduce => {
                let handles = chunk_ranges(dataset.values.len(), self.chunk_size)
                    .into_iter()
                    .map(|range| {
                        let values = Arc::clone(&dataset.values);
                        self.pool.submit(move || {
                            range.fold(0u64, |acc, index| acc.wrapping_add(values[index]))
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(wait_all(handles)?
                    .into_iter()
                    .fold(0u64, |acc, value| acc.wrapping_add(value)))
            }
            ScenarioId::Find => {
                let handles = chunk_ranges(dataset.values.len(), self.chunk_size)
                    .into_iter()
                    .map(|mut range| {
                        let values = Arc::clone(&dataset.values);
                        let needle = dataset.needle;
                        self.pool
                            .submit(move || range.find(|index| values[*index] == needle))
                    })
                    .collect::<Vec<_>>();
                Ok(wait_all(handles)?
                    .into_iter()
                    .flatten()
                    .min()
                    .map_or(u64::MAX, |index| index as u64))
            }
            ScenarioId::InclusiveScan => {
                let ranges = chunk_ranges(dataset.values.len(), self.chunk_size);
                let totals = wait_all(
                    ranges
                        .iter()
                        .cloned()
                        .map(|range| {
                            let values = Arc::clone(&dataset.values);
                            self.pool.submit(move || {
                                range.fold(0u64, |acc, index| acc.wrapping_add(values[index]))
                            })
                        })
                        .collect(),
                )?;

                let mut offsets = Vec::with_capacity(totals.len());
                let mut carry = 0u64;
                for total in totals {
                    offsets.push(carry);
                    carry = carry.wrapping_add(total);
                }

                let handles = ranges
                    .into_iter()
                    .enumerate()
                    .map(|(chunk_index, range)| {
                        let values = Arc::clone(&dataset.values);
                        let offset = offsets[chunk_index];
                        self.pool.submit(move || {
                            let mut acc = offset;
                            let mut output = Vec::with_capacity(range.end - range.start);
                            for index in range {
                                acc = acc.wrapping_add(values[index]);
                                output.push(acc);
                            }
                            output
                        })
                    })
                    .collect::<Vec<_>>();

                Ok(checksum_slice(
                    &wait_all(handles)?.into_iter().flatten().collect::<Vec<_>>(),
                ))
            }
            ScenarioId::Sort => {
                let mut chunks = wait_all(
                    split_chunks(dataset.sort_input(), self.chunk_size)
                        .into_iter()
                        .map(|mut chunk| {
                            self.pool.submit(move || {
                                chunk.sort_unstable();
                                chunk
                            })
                        })
                        .collect(),
                )?;

                while chunks.len() > 1 {
                    let mut next_round = Vec::with_capacity(chunks.len().div_ceil(2));
                    let mut pending = chunks.into_iter();
                    while let Some(left) = pending.next() {
                        if let Some(right) = pending.next() {
                            next_round.push(self.pool.submit(move || merge_sorted(left, right)));
                        } else {
                            next_round.push(self.pool.submit(move || left));
                        }
                    }
                    chunks = wait_all(next_round)?;
                }

                let output = chunks
                    .pop()
                    .expect("threadpool sort should produce one output");
                Ok(checksum_slice(&output))
            }
        }
    }
}

struct RayonBackend {
    pool: ThreadPool,
    chunk_size: usize,
}

impl RayonBackend {
    fn new(config: &BenchmarkConfig) -> Result<Self, BenchmarkError> {
        let pool = ThreadPoolBuilder::new()
            .num_threads(config.worker_count)
            .build()
            .map_err(|error| BenchmarkError::new(error.to_string()))?;
        Ok(Self {
            pool,
            chunk_size: config.chunk_size,
        })
    }

    fn measure(
        &self,
        scenario: ScenarioId,
        dataset: &Dataset,
        config: &BenchmarkConfig,
    ) -> Result<Measurement, BenchmarkError> {
        measure_iterations(config, || self.run_once(scenario, dataset))
    }

    fn run_once(&self, scenario: ScenarioId, dataset: &Dataset) -> Result<u64, BenchmarkError> {
        let chunk_size = self.chunk_size.max(1);
        let values = &dataset.values;

        Ok(match scenario {
            ScenarioId::Transform => self.pool.install(|| {
                let output = values
                    .par_iter()
                    .map(|value| transform_value(*value))
                    .collect::<Vec<_>>();
                checksum_slice(&output)
            }),
            ScenarioId::Reduce => self.pool.install(|| {
                values
                    .par_iter()
                    .copied()
                    .reduce(|| 0u64, |left, right| left.wrapping_add(right))
            }),
            ScenarioId::Find => self.pool.install(|| {
                values
                    .par_iter()
                    .enumerate()
                    .find_first(|(_, value)| **value == dataset.needle)
                    .map_or(u64::MAX, |(index, _)| index as u64)
            }),
            ScenarioId::InclusiveScan => self.pool.install(|| {
                let chunks = values.chunks(chunk_size).collect::<Vec<_>>();
                let totals = chunks
                    .par_iter()
                    .map(|chunk| {
                        chunk
                            .iter()
                            .fold(0u64, |acc, value| acc.wrapping_add(*value))
                    })
                    .collect::<Vec<_>>();

                let mut offsets = Vec::with_capacity(totals.len());
                let mut carry = 0u64;
                for total in totals {
                    offsets.push(carry);
                    carry = carry.wrapping_add(total);
                }

                let outputs = (0..chunks.len())
                    .into_par_iter()
                    .map(|chunk_index| {
                        let mut acc = offsets[chunk_index];
                        let mut output = Vec::with_capacity(chunks[chunk_index].len());
                        for value in chunks[chunk_index] {
                            acc = acc.wrapping_add(*value);
                            output.push(acc);
                        }
                        output
                    })
                    .collect::<Vec<_>>();

                checksum_slice(&outputs.into_iter().flatten().collect::<Vec<_>>())
            }),
            ScenarioId::Sort => self.pool.install(|| {
                let mut output = dataset.sort_input();
                output.par_sort_unstable();
                checksum_slice(&output)
            }),
        })
    }
}

struct CppTaskflowBackend {
    source: PathBuf,
    binary: PathBuf,
    include_root: PathBuf,
}

impl CppTaskflowBackend {
    fn new() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .expect("benchmark manifest should be under the workspace");
        let source = manifest_dir.join("cpp/taskflow_cpu.cpp");
        let binary_dir = workspace_root.join("target/benchmark-drivers");
        let binary = binary_dir.join("taskflow_cpu");
        let include_root = workspace_root.join("taskflow");

        Self {
            source,
            binary,
            include_root,
        }
    }

    fn measure(
        &self,
        scenario: ScenarioId,
        config: &BenchmarkConfig,
    ) -> Result<Measurement, String> {
        self.ensure_driver()?;

        let output = Command::new(&self.binary)
            .arg("--scenario")
            .arg(scenario.as_str())
            .arg("--input-len")
            .arg(config.input_len.to_string())
            .arg("--workers")
            .arg(config.worker_count.to_string())
            .arg("--chunk-size")
            .arg(config.chunk_size.to_string())
            .arg("--warmups")
            .arg(config.warmup_iterations.to_string())
            .arg("--iterations")
            .arg(config.measure_iterations.to_string())
            .arg("--seed")
            .arg(config.seed.to_string())
            .output()
            .map_err(|error| format!("failed to run cpp-taskflow driver: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "cpp-taskflow driver failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        parse_cpp_output(&output.stdout)
    }

    fn ensure_driver(&self) -> Result<(), String> {
        let needs_rebuild = match (
            std::fs::metadata(&self.binary),
            std::fs::metadata(&self.source),
        ) {
            (Ok(binary_meta), Ok(source_meta)) => {
                match (binary_meta.modified(), source_meta.modified()) {
                    (Ok(binary_time), Ok(source_time)) => source_time > binary_time,
                    _ => true,
                }
            }
            _ => true,
        };

        if !needs_rebuild {
            return Ok(());
        }

        if let Some(parent) = self.binary.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create benchmark driver directory: {error}"))?;
        }

        let status = Command::new("c++")
            .arg("-std=c++20")
            .arg("-O3")
            .arg("-DNDEBUG")
            .arg("-I")
            .arg(&self.include_root)
            .arg(&self.source)
            .arg("-o")
            .arg(&self.binary)
            .status()
            .map_err(|error| format!("failed to invoke c++: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "failed to compile cpp-taskflow driver (status {status})"
            ))
        }
    }
}

struct SimpleThreadPool {
    inner: Arc<PoolInner>,
    threads: Vec<thread::JoinHandle<()>>,
}

struct PoolInner {
    queue: Mutex<VecDeque<Job>>,
    ready: Condvar,
    shutdown: AtomicBool,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl SimpleThreadPool {
    fn new(worker_count: usize) -> Self {
        let inner = Arc::new(PoolInner {
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            shutdown: AtomicBool::new(false),
        });

        let mut threads = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let inner_clone = Arc::clone(&inner);
            threads.push(thread::spawn(move || pool_worker(inner_clone)));
        }

        Self { inner, threads }
    }

    fn submit<F, T>(&self, task: F) -> PoolHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let state = Arc::new(PoolHandleState::default());
        let state_clone = Arc::clone(&state);

        self.enqueue(Box::new(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(task))
                .map_err(|payload| BenchmarkError::new(panic_payload_to_string(payload)));
            state_clone.store(result);
        }));

        PoolHandle { state }
    }

    fn enqueue(&self, job: Job) {
        let mut queue = self.inner.queue.lock().expect("threadpool queue poisoned");
        queue.push_back(job);
        drop(queue);
        self.inner.ready.notify_one();
    }
}

impl Drop for SimpleThreadPool {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        self.inner.ready.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn pool_worker(inner: Arc<PoolInner>) {
    loop {
        let job = {
            let mut queue = inner.queue.lock().expect("threadpool queue poisoned");
            loop {
                if let Some(job) = queue.pop_front() {
                    break Some(job);
                }
                if inner.shutdown.load(Ordering::Acquire) {
                    break None;
                }
                queue = inner.ready.wait(queue).expect("threadpool wait poisoned");
            }
        };

        match job {
            Some(job) => job(),
            None => return,
        }
    }
}

struct PoolHandle<T> {
    state: Arc<PoolHandleState<T>>,
}

struct PoolHandleState<T> {
    value: Mutex<Option<Result<T, BenchmarkError>>>,
    ready: Condvar,
}

impl<T> Default for PoolHandleState<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new(None),
            ready: Condvar::new(),
        }
    }
}

impl<T> PoolHandleState<T> {
    fn store(&self, result: Result<T, BenchmarkError>) {
        let mut slot = self.value.lock().expect("pool handle state poisoned");
        *slot = Some(result);
        self.ready.notify_all();
    }
}

impl<T> PoolHandle<T> {
    fn wait(self) -> Result<T, BenchmarkError> {
        let mut slot = self.state.value.lock().expect("pool handle state poisoned");
        while slot.is_none() {
            slot = self
                .state
                .ready
                .wait(slot)
                .expect("pool handle wait poisoned");
        }
        slot.take().expect("pool handle should contain a result")
    }
}

fn wait_all<T>(handles: Vec<PoolHandle<T>>) -> Result<Vec<T>, BenchmarkError> {
    handles.into_iter().map(PoolHandle::wait).collect()
}

fn measure_iterations<F>(
    config: &BenchmarkConfig,
    mut run_once: F,
) -> Result<Measurement, BenchmarkError>
where
    F: FnMut() -> Result<u64, BenchmarkError>,
{
    for _ in 0..config.warmup_iterations {
        let _ = run_once()?;
    }

    let mut samples_ns = Vec::with_capacity(config.measure_iterations);
    let mut checksum = 0u64;

    for _ in 0..config.measure_iterations {
        let start = Instant::now();
        checksum = run_once()?;
        samples_ns.push(start.elapsed().as_nanos());
    }

    Ok(Measurement {
        samples_ns,
        checksum,
    })
}

fn available_entry(
    backend: BackendId,
    scenario: ScenarioId,
    measurement: Measurement,
) -> BenchmarkEntry {
    let min_ns = measurement.samples_ns.iter().copied().min().unwrap_or(0);
    let max_ns = measurement.samples_ns.iter().copied().max().unwrap_or(0);
    let mean_ns = if measurement.samples_ns.is_empty() {
        0
    } else {
        measurement.samples_ns.iter().sum::<u128>() / measurement.samples_ns.len() as u128
    };
    let median_ns = median(&measurement.samples_ns);

    BenchmarkEntry {
        backend,
        scenario,
        available: true,
        reason: None,
        samples_ns: measurement.samples_ns,
        min_ns,
        max_ns,
        median_ns,
        mean_ns,
        checksum: measurement.checksum,
    }
}

fn unavailable_entry(backend: BackendId, scenario: ScenarioId, reason: String) -> BenchmarkEntry {
    BenchmarkEntry {
        backend,
        scenario,
        available: false,
        reason: Some(reason),
        samples_ns: Vec::new(),
        min_ns: 0,
        max_ns: 0,
        median_ns: 0,
        mean_ns: 0,
        checksum: 0,
    }
}

fn median(values: &[u128]) -> u128 {
    if values.is_empty() {
        return 0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}

fn chunk_ranges(len: usize, chunk_size: usize) -> Vec<Range<usize>> {
    if len == 0 {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0usize;
    while start < len {
        let end = start.saturating_add(chunk_size).min(len);
        ranges.push(start..end);
        start = end;
    }
    ranges
}

fn split_chunks(values: Vec<u64>, chunk_size: usize) -> Vec<Vec<u64>> {
    let mut chunks = Vec::new();
    let mut iter = values.into_iter();

    loop {
        let mut chunk = Vec::with_capacity(chunk_size);
        for _ in 0..chunk_size {
            match iter.next() {
                Some(value) => chunk.push(value),
                None => break,
            }
        }
        if chunk.is_empty() {
            break;
        }
        chunks.push(chunk);
    }

    chunks
}

fn merge_sorted(left: Vec<u64>, right: Vec<u64>) -> Vec<u64> {
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut merged = Vec::with_capacity(left.len() + right.len());

    while let (Some(left_value), Some(right_value)) = (left.peek(), right.peek()) {
        if left_value <= right_value {
            merged.push(left.next().expect("left iterator should have a value"));
        } else {
            merged.push(right.next().expect("right iterator should have a value"));
        }
    }

    merged.extend(left);
    merged.extend(right);
    merged
}

fn transform_value(value: u64) -> u64 {
    value.rotate_left(7).wrapping_add(0x9e37_79b9)
}

fn checksum_slice(values: &[u64]) -> u64 {
    values.iter().fold(CHECKSUM_OFFSET, |acc, value| {
        acc.wrapping_mul(CHECKSUM_PRIME) ^ value
    })
}

fn parse_cpp_output(stdout: &[u8]) -> Result<Measurement, String> {
    let text = String::from_utf8_lossy(stdout);
    let mut checksum = None;
    let mut samples_ns = Vec::new();

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(kind) = parts.next() else {
            continue;
        };
        let Some(value) = parts.next() else {
            continue;
        };

        match kind {
            "checksum" => {
                checksum = Some(
                    value
                        .parse::<u64>()
                        .map_err(|error| format!("invalid checksum output: {error}"))?,
                );
            }
            "sample_ns" => {
                samples_ns.push(
                    value
                        .parse::<u128>()
                        .map_err(|error| format!("invalid sample output: {error}"))?,
                );
            }
            _ => {}
        }
    }

    Ok(Measurement {
        samples_ns,
        checksum: checksum
            .ok_or_else(|| "cpp-taskflow driver did not return checksum".to_string())?,
    })
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn next_arg<I>(args: &mut I, flag: &str) -> Result<String, BenchmarkError>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| BenchmarkError::new(format!("missing value for `{flag}`")))
}

fn parse_usize<I>(args: &mut I, flag: &str) -> Result<usize, BenchmarkError>
where
    I: Iterator<Item = String>,
{
    next_arg(args, flag)?
        .parse::<usize>()
        .map_err(|error| BenchmarkError::new(format!("invalid value for `{flag}`: {error}")))
}

fn parse_u64<I>(args: &mut I, flag: &str) -> Result<u64, BenchmarkError>
where
    I: Iterator<Item = String>,
{
    next_arg(args, flag)?
        .parse::<u64>()
        .map_err(|error| BenchmarkError::new(format!("invalid value for `{flag}`: {error}")))
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "benchmark task panicked".to_string()
    }
}

fn help_text() -> String {
    [
        "Usage: cargo run -p benchmarks -- [options]",
        "  --backends flow,threadpool,rayon,cpp-taskflow",
        "  --scenarios transform,reduce,find,inclusive-scan,sort",
        "  --input-len <usize>",
        "  --workers <usize>",
        "  --chunk-size <usize>",
        "  --warmups <usize>",
        "  --iterations <usize>",
        "  --seed <u64>",
        "  --format table|json",
    ]
    .join("\n")
}

fn _assert_cpp_source_exists(path: &Path) {
    debug_assert!(path.exists(), "cpp benchmark source should exist");
}
