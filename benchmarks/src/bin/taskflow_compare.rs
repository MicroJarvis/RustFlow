use std::collections::HashMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::os::raw::{c_int, c_uint};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use flow_algorithms::{
    DataPipeline, DynamicPartitioner, Executor, Flow, ParallelForExt, ParallelForOptions,
    Partitioner, PartitionState, Pipe, PipeType, Pipeline, RuntimeCtx, TaskHandle,
    parallel_inclusive_scan, parallel_reduce, parallel_sort, parallel_transform,
};

unsafe extern "C" {
    fn rand() -> c_int;
    fn srand(seed: c_uint);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Profile {
    Report,
}

#[derive(Clone, Debug)]
struct Config {
    threads: usize,
    rounds: usize,
    profile: Profile,
    output_dir: PathBuf,
    selected_cases: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
struct PointSpec {
    label: String,
    value: usize,
}

#[derive(Clone, Debug)]
struct PointResult {
    size: String,
    rustflow_ms: Option<f64>,
    taskflow_ms: Option<f64>,
    note: Option<String>,
}

#[derive(Clone, Debug)]
struct CaseResult {
    id: &'static str,
    title: &'static str,
    note: Option<String>,
    points: Vec<PointResult>,
}

#[derive(Clone)]
struct TaskflowHarness {
    workspace_root: PathBuf,
    taskflow_root: PathBuf,
    driver_dir: PathBuf,
    binary_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct LevelGraph {
    length: usize,
    out_edges: Vec<Vec<Vec<usize>>>,
    in_edges: Vec<Vec<Vec<(usize, usize)>>>,
}

#[derive(Clone)]
struct ShuffleRng {
    state: u64,
}

impl ShuffleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        if values.len() < 2 {
            return;
        }
        for index in (1..values.len()).rev() {
            let choice = (self.next_u64() % (index as u64 + 1)) as usize;
            values.swap(index, choice);
        }
    }
}

fn main() {
    let config = parse_args(env::args().skip(1).collect());
    let harness = TaskflowHarness::new();

    match run_suite(&config, &harness) {
        Ok(results) => {
            let report = render_markdown(&config, &results);
            let report_path = config.output_dir.join("taskflow_vs_rustflow_report.md");
            if let Some(parent) = report_path.parent() {
                fs::create_dir_all(parent).expect("failed to create report directory");
            }
            fs::write(&report_path, report).expect("failed to write markdown report");
            println!("{}", report_path.display());
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn parse_args(args: Vec<String>) -> Config {
    let mut threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4)
        .max(1);
    let mut rounds = 1usize;
    let mut output_dir = workspace_root().join("benchmarks/reports/taskflow_compare");
    let mut selected_cases = None;

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--threads" => {
                index += 1;
                threads = args
                    .get(index)
                    .expect("missing value for --threads")
                    .parse()
                    .expect("invalid value for --threads");
            }
            "--rounds" => {
                index += 1;
                rounds = args
                    .get(index)
                    .expect("missing value for --rounds")
                    .parse()
                    .expect("invalid value for --rounds");
            }
            "--output-dir" => {
                index += 1;
                output_dir =
                    PathBuf::from(args.get(index).expect("missing value for --output-dir"));
            }
            "--cases" => {
                index += 1;
                selected_cases = Some(
                    args.get(index)
                        .expect("missing value for --cases")
                        .split(',')
                        .map(|token| token.trim().to_string())
                        .filter(|token| !token.is_empty())
                        .collect(),
                );
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => panic!("unknown argument `{other}`"),
        }
        index += 1;
    }

    Config {
        threads,
        rounds: rounds.max(1),
        profile: Profile::Report,
        output_dir,
        selected_cases,
    }
}

fn print_help() {
    println!("Usage: cargo run -p benchmarks --release --bin taskflow_compare -- [options]");
    println!("  --threads <usize>      number of worker threads");
    println!("  --rounds <usize>       benchmark rounds per point");
    println!("  --output-dir <path>    report output directory");
    println!("  --cases <csv>          run only a subset of benchmark ids");
}

impl TaskflowHarness {
    fn new() -> Self {
        let workspace_root = workspace_root();
        let taskflow_root = workspace_root.join("taskflow");
        let driver_dir = workspace_root.join("target/taskflow-compare/drivers");
        let binary_dir = workspace_root.join("target/taskflow-compare/bin");
        Self {
            workspace_root,
            taskflow_root,
            driver_dir,
            binary_dir,
        }
    }

    fn run_case(
        &self,
        case_id: &str,
        source: String,
        threads: usize,
        rounds: usize,
        points: &[PointSpec],
    ) -> Result<HashMap<String, f64>, String> {
        if points.is_empty() {
            return Ok(HashMap::new());
        }

        let binary = self.ensure_driver(case_id, source)?;
        let mut command = Command::new(binary);
        command.arg(threads.to_string()).arg(rounds.to_string());
        for point in points {
            command.arg(point.value.to_string());
        }

        let output = command
            .output()
            .map_err(|error| format!("failed to run taskflow driver `{case_id}`: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "taskflow driver `{case_id}` failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut result = HashMap::new();
        for line in stdout.lines() {
            let mut parts = line.split_whitespace();
            let Some(kind) = parts.next() else {
                continue;
            };
            if kind != "point" {
                continue;
            }
            let Some(label) = parts.next() else {
                continue;
            };
            let Some(ms) = parts.next() else {
                continue;
            };
            let value = ms
                .parse::<f64>()
                .map_err(|error| format!("invalid taskflow output for `{case_id}`: {error}"))?;
            result.insert(label.to_string(), value);
        }

        Ok(result)
    }

    fn ensure_driver(&self, case_id: &str, source: String) -> Result<PathBuf, String> {
        fs::create_dir_all(&self.driver_dir)
            .map_err(|error| format!("failed to create driver directory: {error}"))?;
        fs::create_dir_all(&self.binary_dir)
            .map_err(|error| format!("failed to create binary directory: {error}"))?;

        let source_path = self.driver_dir.join(format!("{case_id}.cpp"));
        let binary_path = self.binary_dir.join(case_id);

        let write_source = match fs::read_to_string(&source_path) {
            Ok(existing) => existing != source,
            Err(_) => true,
        };
        if write_source {
            fs::write(&source_path, source)
                .map_err(|error| format!("failed to write driver source `{case_id}`: {error}"))?;
        }

        let needs_rebuild = match (fs::metadata(&source_path), fs::metadata(&binary_path)) {
            (Ok(source_meta), Ok(binary_meta)) => {
                match (source_meta.modified(), binary_meta.modified()) {
                    (Ok(source_time), Ok(binary_time)) => source_time > binary_time,
                    _ => true,
                }
            }
            _ => true,
        };

        if !needs_rebuild {
            return Ok(binary_path);
        }

        let output = Command::new("c++")
            .arg("-std=c++20")
            .arg("-O3")
            .arg("-DNDEBUG")
            .arg("-pthread")
            .arg("-I")
            .arg(&self.taskflow_root)
            .arg(&source_path)
            .arg("-o")
            .arg(&binary_path)
            .output()
            .map_err(|error| format!("failed to invoke c++ for `{case_id}`: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "failed to compile taskflow driver `{case_id}`: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(binary_path)
    }

    fn benchmark_cpp(&self, relative: &str) -> PathBuf {
        self.workspace_root
            .join("taskflow/benchmarks")
            .join(relative)
    }
}

fn run_suite(config: &Config, harness: &TaskflowHarness) -> Result<Vec<CaseResult>, String> {
    let mut results = Vec::new();

    if wants_case(config, "async_task") {
        eprintln!("[taskflow_compare] async_task");
        results.push(run_async_task_case(config, harness));
    }
    if wants_case(config, "binary_tree") {
        eprintln!("[taskflow_compare] binary_tree");
        results.push(run_binary_tree_case(config, harness));
    }
    if wants_case(config, "black_scholes") {
        eprintln!("[taskflow_compare] black_scholes");
        results.push(run_black_scholes_case(config, harness));
    }
    if wants_case(config, "data_pipeline") {
        eprintln!("[taskflow_compare] data_pipeline");
        results.push(run_data_pipeline_case(config, harness));
    }
    if wants_case(config, "embarrassing_parallelism") {
        eprintln!("[taskflow_compare] embarrassing_parallelism");
        results.push(run_embarrassing_parallelism_case(config, harness));
    }
    if wants_case(config, "fibonacci") {
        eprintln!("[taskflow_compare] fibonacci");
        results.push(run_fibonacci_case(config, harness));
    }
    if wants_case(config, "for_each") {
        eprintln!("[taskflow_compare] for_each");
        results.push(run_for_each_case(config, harness));
    }
    if wants_case(config, "graph_pipeline") {
        eprintln!("[taskflow_compare] graph_pipeline");
        results.push(run_graph_pipeline_case(config, harness));
    }
    if wants_case(config, "graph_traversal") {
        eprintln!("[taskflow_compare] graph_traversal");
        results.push(run_graph_traversal_case(config, harness));
    }
    if wants_case(config, "hetero_traversal") {
        eprintln!("[taskflow_compare] hetero_traversal");
        results.push(skipped_case(
            "hetero_traversal",
            "Heterogeneous Traversal",
            "Skipped: Taskflow benchmark is CUDA-based and RustFlow currently has no heterogeneous execution backend.",
        ));
    }
    if wants_case(config, "integrate") {
        eprintln!("[taskflow_compare] integrate");
        results.push(run_integrate_case(config, harness));
    }
    if wants_case(config, "linear_chain") {
        eprintln!("[taskflow_compare] linear_chain");
        results.push(run_linear_chain_case(config, harness));
    }
    if wants_case(config, "linear_pipeline") {
        eprintln!("[taskflow_compare] linear_pipeline");
        results.push(run_linear_pipeline_case(config, harness));
    }
    if wants_case(config, "mandelbrot") {
        eprintln!("[taskflow_compare] mandelbrot");
        results.push(run_mandelbrot_case(config, harness));
    }
    if wants_case(config, "matrix_multiplication") {
        eprintln!("[taskflow_compare] matrix_multiplication");
        results.push(run_matrix_multiplication_case(config, harness));
    }
    if wants_case(config, "merge_sort") {
        eprintln!("[taskflow_compare] merge_sort");
        results.push(run_merge_sort_case(config, harness));
    }
    if wants_case(config, "mnist") {
        eprintln!("[taskflow_compare] mnist");
        results.push(skipped_case(
            "mnist",
            "MNIST",
            "Skipped: Taskflow benchmark requires external Kaggle MNIST data files that are not present in this workspace.",
        ));
    }
    if wants_case(config, "nqueens") {
        eprintln!("[taskflow_compare] nqueens");
        results.push(run_nqueens_case(config, harness));
    }
    if wants_case(config, "primes") {
        eprintln!("[taskflow_compare] primes");
        results.push(run_primes_case(config, harness));
    }
    if wants_case(config, "reduce_sum") {
        eprintln!("[taskflow_compare] reduce_sum");
        results.push(run_reduce_sum_case(config, harness));
    }
    if wants_case(config, "scan") {
        eprintln!("[taskflow_compare] scan");
        results.push(run_scan_case(config, harness));
    }
    if wants_case(config, "skynet") {
        eprintln!("[taskflow_compare] skynet");
        results.push(run_skynet_case(config, harness));
    }
    if wants_case(config, "sort") {
        eprintln!("[taskflow_compare] sort");
        results.push(run_sort_case(config, harness));
    }
    if wants_case(config, "thread_pool") {
        eprintln!("[taskflow_compare] thread_pool");
        results.push(run_thread_pool_case(config, harness));
    }
    if wants_case(config, "wavefront") {
        eprintln!("[taskflow_compare] wavefront");
        results.push(run_wavefront_case(config, harness));
    }

    Ok(results)
}

fn wants_case(config: &Config, case_id: &str) -> bool {
    config
        .selected_cases
        .as_ref()
        .map(|selected| selected.iter().any(|item| item == case_id))
        .unwrap_or(true)
}

fn skipped_case(id: &'static str, title: &'static str, note: &'static str) -> CaseResult {
    CaseResult {
        id,
        title,
        note: Some(note.to_string()),
        points: Vec::new(),
    }
}

fn render_markdown(config: &Config, cases: &[CaseResult]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# RustFlow vs Taskflow Benchmark Report");
    let _ = writeln!(out);
    let _ = writeln!(out, "- Date: 2026-03-31");
    let _ = writeln!(out, "- Profile: {:?}", config.profile);
    let _ = writeln!(out, "- Threads: {}", config.threads);
    let _ = writeln!(out, "- Rounds per point: {}", config.rounds);
    let _ = writeln!(
        out,
        "- Ratio column: RustFlow ms / Taskflow ms (smaller is better)"
    );
    let _ = writeln!(
        out,
        "- Method note: benchmark structure matches `taskflow/benchmarks`, but very large default sweeps are capped in this report profile for practical execution."
    );
    let _ = writeln!(out);

    for case in cases {
        let _ = writeln!(out, "## {}", case.title);
        let _ = writeln!(out);
        let _ = writeln!(out, "- Benchmark id: `{}`", case.id);
        if let Some(note) = &case.note {
            let _ = writeln!(out, "- Note: {note}");
        }
        let _ = writeln!(out);

        if !case.points.is_empty() {
            let _ = writeln!(
                out,
                "| Size | RustFlow (ms) | Taskflow (ms) | Ratio | Note |"
            );
            let _ = writeln!(out, "| ---: | ---: | ---: | ---: | --- |");
            for point in &case.points {
                let ratio = match (point.rustflow_ms, point.taskflow_ms) {
                    (Some(rust), Some(taskflow)) if taskflow > 0.0 => {
                        format!("{:.3}", rust / taskflow)
                    }
                    _ => "-".to_string(),
                };
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    point.size,
                    format_ms(point.rustflow_ms),
                    format_ms(point.taskflow_ms),
                    ratio,
                    point.note.clone().unwrap_or_else(|| "-".to_string())
                );
            }
            let _ = writeln!(out);
        }
    }

    out
}

fn format_ms(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "-".to_string())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("benchmarks crate should be under the workspace")
        .to_path_buf()
}

fn cpp_string_literal(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn driver_prelude() -> &'static str {
    r#"#include <cstdlib>
#include <iostream>
#include <stdexcept>
#include <string>

static size_t parse_size(const char* value) {
  return static_cast<size_t>(std::stoull(value));
}

static unsigned parse_unsigned(const char* value) {
  return static_cast<unsigned>(std::stoul(value));
}

static void print_point(size_t label, double micros, unsigned rounds) {
  std::cout << "point " << label << ' ' << (micros / rounds / 1e3) << '\n';
}

"#
}

fn taskflow_map_note(
    taskflow_result: Result<HashMap<String, f64>, String>,
) -> (Option<HashMap<String, f64>>, Option<String>) {
    match taskflow_result {
        Ok(map) => (Some(map), None),
        Err(error) => (None, Some(error)),
    }
}

fn compare_case<F>(
    id: &'static str,
    title: &'static str,
    base_note: Option<String>,
    points: &[PointSpec],
    taskflow_points: Option<HashMap<String, f64>>,
    taskflow_note: Option<String>,
    mut rustflow: F,
) -> CaseResult
where
    F: FnMut(usize) -> Result<f64, String>,
{
    let mut note = base_note;
    if let Some(extra) = taskflow_note {
        note = Some(match note {
            Some(existing) => format!("{existing} Taskflow driver note: {extra}"),
            None => format!("Taskflow driver note: {extra}"),
        });
    }

    let mut results = Vec::with_capacity(points.len());
    for point in points {
        let rustflow_ms = match rustflow(point.value) {
            Ok(value) => Some(value),
            Err(error) => {
                results.push(PointResult {
                    size: point.label.clone(),
                    rustflow_ms: None,
                    taskflow_ms: taskflow_points
                        .as_ref()
                        .and_then(|values| values.get(&point.label).copied()),
                    note: Some(error),
                });
                continue;
            }
        };

        let taskflow_ms = taskflow_points
            .as_ref()
            .and_then(|values| values.get(&point.label).copied());

        let point_note = if taskflow_points.is_some() && taskflow_ms.is_none() {
            Some("Taskflow point missing from driver output".to_string())
        } else {
            None
        };

        results.push(PointResult {
            size: point.label.clone(),
            rustflow_ms,
            taskflow_ms,
            note: point_note,
        });
    }

    CaseResult {
        id,
        title,
        note,
        points: results,
    }
}

fn average_ms<F, T>(rounds: usize, mut operation: F) -> Result<f64, String>
where
    F: FnMut() -> Result<T, String>,
{
    let mut total = 0.0;
    for _ in 0..rounds {
        let start = Instant::now();
        let value = operation()?;
        black_box(value);
        total += start.elapsed().as_secs_f64() * 1_000.0;
    }
    Ok(total / rounds as f64)
}

fn average_reported_ms<F>(rounds: usize, mut operation: F) -> Result<f64, String>
where
    F: FnMut() -> Result<f64, String>,
{
    let mut total = 0.0;
    for _ in 0..rounds {
        total += operation()?;
    }
    Ok(total / rounds as f64)
}

fn checksum_u8(values: &[u8]) -> u64 {
    values
        .iter()
        .fold(1_469_598_103_934_665_603u64, |acc, value| {
            acc.wrapping_mul(1_099_511_628_211)
                .wrapping_add(*value as u64)
        })
}

fn checksum_i32(values: &[i32]) -> u64 {
    values
        .iter()
        .fold(1_469_598_103_934_665_603u64, |acc, value| {
            acc.wrapping_mul(1_099_511_628_211)
                .wrapping_add(*value as u32 as u64)
        })
}

fn checksum_f64(values: &[f64]) -> u64 {
    values
        .iter()
        .fold(1_469_598_103_934_665_603u64, |acc, value| {
            acc.wrapping_mul(1_099_511_628_211)
                .wrapping_add(value.to_bits())
        })
}

fn checksum_f32(values: &[f32]) -> u64 {
    values
        .iter()
        .fold(1_469_598_103_934_665_603u64, |acc, value| {
            acc.wrapping_mul(1_099_511_628_211)
                .wrapping_add(value.to_bits() as u64)
        })
}

fn auto_chunk_size_for_workers(len: usize, workers: usize) -> usize {
    let target_chunks = workers.max(1).saturating_mul(4).max(1);
    len.div_ceil(target_chunks).max(1)
}

#[derive(Clone, Copy)]
struct SharedMutPtr<T>(*mut T);

unsafe impl<T: Send> Send for SharedMutPtr<T> {}
unsafe impl<T: Send> Sync for SharedMutPtr<T> {}

impl<T> SharedMutPtr<T> {
    fn from_slice(values: &mut [T]) -> Self {
        Self(values.as_mut_ptr())
    }

    unsafe fn update<F, R>(self, index: usize, mut update: F) -> R
    where
        F: FnMut(&mut T) -> R,
    {
        unsafe { update(&mut *self.0.add(index)) }
    }

    unsafe fn write(self, index: usize, value: T) {
        unsafe {
            self.0.add(index).write(value);
        }
    }
}

fn pow2_points(start_exp: usize, end_exp: usize) -> Vec<PointSpec> {
    (start_exp..=end_exp)
        .map(|exponent| {
            let value = 1usize << exponent;
            PointSpec {
                label: value.to_string(),
                value,
            }
        })
        .collect()
}

fn stepped_points(start: usize, end: usize, step: usize) -> Vec<PointSpec> {
    let mut points = Vec::new();
    let mut value = start;
    while value <= end {
        points.push(PointSpec {
            label: value.to_string(),
            value,
        });
        value += step;
    }
    points
}

fn decimal_points(max_power: u32) -> Vec<PointSpec> {
    (1..=max_power)
        .map(|power| {
            let value = 10usize.pow(power);
            PointSpec {
                label: value.to_string(),
                value,
            }
        })
        .collect()
}

fn graph_pipeline_points() -> Vec<PointSpec> {
    (1..=61)
        .step_by(15)
        .map(|value| {
            let graph = LevelGraph::new(value, value);
            PointSpec {
                label: graph.graph_size().to_string(),
                value,
            }
        })
        .collect()
}

fn graph_traversal_points() -> Vec<PointSpec> {
    (0..=6)
        .map(|power| {
            let value = 1usize << power;
            let graph = LevelGraph::new(value, value);
            PointSpec {
                label: graph.graph_size().to_string(),
                value,
            }
        })
        .collect()
}

fn wavefront_points() -> Vec<PointSpec> {
    let mut points = Vec::new();
    let mut size = 8usize;
    while size <= 256 {
        let mb = size.div_ceil(8);
        let nb = size.div_ceil(8);
        points.push(PointSpec {
            label: (mb * nb).to_string(),
            value: size,
        });
        size <<= 1;
    }
    points
}

fn c_rand_reset(seed: u32) {
    unsafe {
        srand(seed as c_uint);
    }
}

fn c_rand_value() -> i32 {
    unsafe { rand() as i32 }
}

fn bench_dummy(index: usize) -> f64 {
    let mut sink = 0.0f64;
    let mut x = (((index as u64).wrapping_mul(1_315_423_911) ^ ((index as u64) >> 3)) as f64) + 1.0;
    for _ in 0..32 {
        x = x.sin() * 1.0000001 + x.cos();
        sink += x;
    }
    sink
}

fn linear_pipeline_work() {
    thread::sleep(Duration::from_micros(10));
}

fn work_int(value: &mut i32) {
    for _ in 0..100 {
        let next = (*value as f64).cos() * *value as f64 + 1.0;
        *value = next.powi(5) as i32 % 2_147_483_647;
    }
}

fn work_vector(value: &mut [i32]) {
    for _ in 0..100 {
        let next = (value[0] as f64).cos() * value[0] as f64 + 1.0;
        value[0] = next.powi(5) as i32 % 2_147_483_647;
    }
}

fn work_float(value: &mut f32) {
    for _ in 0..100 {
        let next = (*value as f64).cos() * *value as f64 + 1.0;
        *value = next.powi(4) as f32;
    }
}

fn work_string(value: &mut String) {
    for _ in 0..50 {
        *value = (value.parse::<i32>().unwrap_or_default() + 1).to_string();
    }
}

#[derive(Clone, Copy)]
struct BlackScholesOption {
    s: f32,
    strike: f32,
    rate: f32,
    volatility: f32,
    time: f32,
    otype: i32,
}

fn cndf(mut input_x: f32) -> f32 {
    let sign = if input_x < 0.0 {
        input_x = -input_x;
        1
    } else {
        0
    };

    let x_input = input_x;
    let exp_values = (-0.5f32 * input_x * input_x).exp();
    let x_nprime_of_x = exp_values * 0.398_942_3f32;

    let mut x_k2 = 0.231_641_9f32 * x_input;
    x_k2 = 1.0 / (1.0 + x_k2);
    let x_k2_2 = x_k2 * x_k2;
    let x_k2_3 = x_k2_2 * x_k2;
    let x_k2_4 = x_k2_3 * x_k2;
    let x_k2_5 = x_k2_4 * x_k2;

    let x_local_1 = x_k2 * 0.319_381_53f32;
    let mut x_local_2 = x_k2_2 * -0.356_563_78f32;
    x_local_2 += x_k2_3 * 1.781_477_9f32;
    x_local_2 += x_k2_4 * -1.821_255_9f32;
    x_local_2 += x_k2_5 * 1.330_274_5f32;

    let x_local = 1.0 - (x_local_1 + x_local_2) * x_nprime_of_x;
    if sign == 1 { 1.0 - x_local } else { x_local }
}

fn black_scholes_price(option: BlackScholesOption) -> f32 {
    let x_sqrt_time = option.time.sqrt();
    let x_log_term = (option.s / option.strike).ln();
    let x_power_term = option.volatility * option.volatility * 0.5;
    let mut x_d1 = (option.rate + x_power_term) * option.time + x_log_term;
    let x_den = option.volatility * x_sqrt_time;
    x_d1 /= x_den;
    let x_d2 = x_d1 - x_den;

    let nof_xd1 = cndf(x_d1);
    let nof_xd2 = cndf(x_d2);
    let future_value_x = option.strike * (-(option.rate) * option.time).exp();

    if option.otype == 0 {
        option.s * nof_xd1 - future_value_x * nof_xd2
    } else {
        future_value_x * (1.0 - nof_xd2) - option.s * (1.0 - nof_xd1)
    }
}

fn run_for_each_flow(executor: &Executor, values: &mut [f64]) -> Result<(), String> {
    if values.is_empty() {
        return Ok(());
    }

    let flow = Flow::new();
    let values_ptr = SharedMutPtr::from_slice(values);
    let options = ParallelForOptions::default().with_chunk_size(auto_chunk_size_for_workers(
        values.len(),
        executor.num_workers(),
    ));

    let _tasks = flow.parallel_for(0..values.len(), options, move |index| unsafe {
        values_ptr.update(index, |value| {
            *value = value.tan();
        });
    });

    executor
        .run(&flow)
        .wait()
        .map_err(|error| error.to_string())
}

fn build_black_scholes_flow(
    input: Arc<[BlackScholesOption]>,
    prices: &mut [f32],
    worker_count: usize,
) -> Flow {
    let flow = Flow::new();

    if input.is_empty() {
        return flow;
    }

    assert_eq!(
        prices.len(),
        input.len(),
        "black-scholes output buffer length must match the input length"
    );

    let prices_ptr = SharedMutPtr::from_slice(prices);
    let options = ParallelForOptions::default()
        .with_chunk_size(auto_chunk_size_for_workers(input.len(), worker_count));

    let _tasks = flow.parallel_for(0..input.len(), options, {
        let input = Arc::clone(&input);
        move |index| unsafe {
            prices_ptr.write(index, black_scholes_price(input[index]));
        }
    });

    flow
}

fn generate_black_scholes_options(num_options: usize) -> Arc<[BlackScholesOption]> {
    let count = num_options.max(1);
    let mut options = Vec::with_capacity(count);
    for _ in 0..count {
        options.push(BlackScholesOption {
            s: (c_rand_value() % 200) as f32,
            strike: (c_rand_value() % 200) as f32,
            rate: 0.1,
            volatility: (c_rand_value() % 100) as f32 / 100.0,
            time: (c_rand_value() % 100) as f32 / 100.0,
            otype: if c_rand_value() % 2 != 0 { 1 } else { 0 },
        });
        let _ = c_rand_value();
    }
    options.into()
}

impl LevelGraph {
    fn new(length: usize, levels: usize) -> Self {
        c_rand_reset(0);
        let mut shuffle_rng = ShuffleRng::new(0);
        let mut out_edges = vec![vec![Vec::new(); length]; levels];
        let mut in_edges = vec![vec![Vec::new(); length]; levels];

        for level in 0..levels {
            let mut next_level_nodes = (0..length).collect::<Vec<_>>();
            shuffle_rng.shuffle(&mut next_level_nodes);

            let mut start = 0usize;
            let mut reshuffle = false;
            for index in 0..length {
                let edge_num = 1 + (c_rand_value().unsigned_abs() as usize % 4);
                let end = if start + edge_num >= length {
                    reshuffle = true;
                    length
                } else {
                    start + edge_num
                };
                out_edges[level][index] = next_level_nodes[start..end].to_vec();

                let _chosen = (c_rand_value() as f64) / (i32::MAX as f64 + 1.0);
                if reshuffle {
                    shuffle_rng.shuffle(&mut next_level_nodes);
                    start = 0;
                    reshuffle = false;
                } else {
                    start = end;
                }
            }
        }

        for level in 0..levels.saturating_sub(1) {
            for index in 0..length {
                for (edge_index, destination) in out_edges[level][index].iter().copied().enumerate()
                {
                    in_edges[level + 1][destination].push((index, edge_index));
                }
            }
        }

        Self {
            length,
            out_edges,
            in_edges,
        }
    }

    fn graph_size(&self) -> usize {
        self.out_edges
            .iter()
            .map(|level| level.iter().map(|edges| 1 + edges.len()).sum::<usize>())
            .sum()
    }

    fn uid(&self, level: usize, index: usize) -> usize {
        index + level * self.length
    }
}

fn graph_pipeline_work(seed: i32) -> i32 {
    let matrix_size = 16usize;
    let mut result = vec![vec![0i32; matrix_size]; matrix_size];

    for i in 0..matrix_size {
        for j in 0..matrix_size {
            let left = i as i32 + j as i32 - seed % 10;
            let right = i as i32 - j as i32 + seed % 10;
            for k in 0..matrix_size {
                let a = i as i32 + k as i32 - seed % 10;
                let b = k as i32 - j as i32 + seed % 10;
                result[i][j] += a * b;
            }
            let _ = left;
            let _ = right;
        }
    }

    let mut value = 0i32;
    for row in result {
        for entry in row {
            value += entry;
        }
    }
    value.rem_euclid(999)
}

fn run_async_task_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = pow2_points(0, 16);
    let taskflow_driver = driver_async_task(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "async_task",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "async_task",
        "Async Task",
        Some("Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^21).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |num_tasks| {
            average_ms(config.rounds, || {
                let counter = Arc::new(AtomicUsize::new(0));
                for _ in 0..num_tasks {
                    let counter = Arc::clone(&counter);
                    executor.silent_async(move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                    });
                }
                executor.wait_for_all();
                let value = counter.load(Ordering::Relaxed);
                if value != num_tasks {
                    return Err(format!("async task count mismatch: expected {num_tasks}, got {value}"));
                }
                Ok(value)
            })
        },
    )
}

fn run_binary_tree_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = (1..=14)
        .map(|layers| PointSpec {
            label: (1usize << layers).to_string(),
            value: layers,
        })
        .collect::<Vec<_>>();
    let taskflow_driver = driver_binary_tree(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "binary_tree",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "binary_tree",
        "Binary Tree",
        Some(
            "Report profile caps the original Taskflow sweep at 14 layers (original default: 25)."
                .to_string(),
        ),
        &points,
        taskflow_points,
        taskflow_note,
        |layers| {
            average_ms(config.rounds, || {
                let flow = Flow::new();
                let total = 1usize << layers;
                let counter = Arc::new(AtomicUsize::new(0));
                let mut tasks = Vec::with_capacity(total);
                tasks.push(flow.placeholder());
                for _ in 1..total {
                    let counter = Arc::clone(&counter);
                    tasks.push(flow.spawn(move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }));
                }
                for index in 1..total {
                    let left = index << 1;
                    let right = left + 1;
                    if left < total && right < total {
                        tasks[index].precede([tasks[left].clone(), tasks[right].clone()]);
                    }
                }
                executor
                    .run(&flow)
                    .wait()
                    .map_err(|error| error.to_string())?;
                let count = counter.load(Ordering::Relaxed);
                if count + 1 != total {
                    return Err(format!(
                        "binary tree counter mismatch: expected {}, got {}",
                        total - 1,
                        count
                    ));
                }
                Ok(count)
            })
        },
    )
}

fn run_black_scholes_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(1_000, 3_000, 1_000);
    let taskflow_driver = driver_black_scholes(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "black_scholes",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);
    c_rand_reset(1);

    compare_case(
        "black_scholes",
        "Black-Scholes",
        Some("Report profile caps the original Taskflow sweep at 3,000 options (original default: 10,000).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |num_options| {
            let options = generate_black_scholes_options(num_options);
            let mut prices = vec![0.0f32; options.len()];
            let flow =
                build_black_scholes_flow(Arc::clone(&options), &mut prices, executor.num_workers());

            average_reported_ms(config.rounds, || {
                let start = Instant::now();
                for _ in 0..options.len().max(1) {
                    executor
                        .run(&flow)
                        .wait()
                        .map_err(|error| error.to_string())?;
                }
                let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;

                black_box(checksum_f32(&prices));
                Ok(elapsed_ms)
            })
        },
    )
}

fn run_data_pipeline_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = pow2_points(1, 10);
    let taskflow_driver = driver_data_pipeline(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "data_pipeline",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "data_pipeline",
        "Data Pipeline",
        Some("Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let pipeline = DataPipeline::builder(8)
                    .source(PipeType::Serial, move |ctx| -> i32 {
                        if ctx.token() == size {
                            ctx.stop();
                            return 0;
                        }
                        ctx.token() as i32
                    })
                    .stage(PipeType::Serial, |value: &mut i32, _ctx| -> f32 {
                        work_int(value);
                        *value as f32
                    })
                    .stage(PipeType::Serial, |value: &mut f32, _ctx| -> i32 {
                        work_float(value);
                        *value as i32
                    })
                    .stage(PipeType::Serial, |value: &mut i32, _ctx| -> String {
                        work_int(value);
                        value.to_string()
                    })
                    .stage(PipeType::Serial, |value: &mut String, _ctx| -> i32 {
                        work_string(value);
                        value.parse::<i32>().unwrap_or_default()
                    })
                    .stage(PipeType::Serial, |value: &mut i32, _ctx| -> Vec<i32> {
                        work_int(value);
                        vec![*value]
                    })
                    .stage(PipeType::Serial, |value: &mut Vec<i32>, _ctx| -> i32 {
                        work_vector(value);
                        value[0]
                    })
                    .sink(PipeType::Serial, |value: &mut i32, _ctx| {
                        work_int(value);
                    });

                pipeline.run(&executor).wait().map_err(|error| error.to_string())?;
                Ok(pipeline.num_tokens())
            })
        },
    )
}

fn run_embarrassing_parallelism_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = pow2_points(0, 16);
    let taskflow_driver = driver_embarrassing_parallelism(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "embarrassing_parallelism",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "embarrassing_parallelism",
        "Embarrassing Parallelism",
        Some("Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^20).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |num_tasks| {
            average_ms(config.rounds, || {
                let flow = Flow::new();
                let sum = Arc::new(AtomicU64::new(0));
                for index in 0..num_tasks {
                    let sum = Arc::clone(&sum);
                    flow.spawn(move || {
                        let value = bench_dummy(index).to_bits();
                        sum.fetch_add(value, Ordering::Relaxed);
                    });
                }
                executor.run(&flow).wait().map_err(|error| error.to_string())?;
                Ok(sum.load(Ordering::Relaxed))
            })
        },
    )
}

fn run_fibonacci_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(1, 20, 1);
    let taskflow_driver = driver_fibonacci(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "fibonacci",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "fibonacci",
        "Fibonacci",
        Some("Report profile caps the original Taskflow sweep at `fib(20)` (original default: `fib(40)`).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |value| {
            average_ms(config.rounds, || {
                let result = fibonacci_flow(&executor, value)?;
                Ok(result)
            })
        },
    )
}

fn run_for_each_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = decimal_points(5);
    let taskflow_driver = driver_for_each(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "for_each",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);
    c_rand_reset(1);

    compare_case(
        "for_each",
        "For Each",
        Some("Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            let mut input = vec![0.0f64; size];

            average_reported_ms(config.rounds, || {
                for value in &mut input {
                    *value = c_rand_value() as f64;
                }

                let start = Instant::now();
                run_for_each_flow(&executor, &mut input)?;
                let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;

                black_box(checksum_f64(&input));
                Ok(elapsed_ms)
            })
        },
    )
}

fn run_graph_pipeline_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = graph_pipeline_points();
    let taskflow_driver = driver_graph_pipeline(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "graph_pipeline",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "graph_pipeline",
        "Graph Pipeline",
        Some("Taskflow defaults are preserved: pipes=8, num_lines=8. Report profile caps the graph dimension at 61 (original default: 451).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |dimension| {
            let graph = Arc::new(LevelGraph::new(dimension, dimension));
            let values = Arc::new(
                (0..dimension * dimension)
                    .map(|_| AtomicI32::new(0))
                    .collect::<Vec<_>>(),
            );
            average_ms(config.rounds, || {
                let buffer = Arc::new((0..8).map(|_| AtomicI32::new(0)).collect::<Vec<_>>());
                let total_nodes = dimension * dimension;
                let graph_for_input = Arc::clone(&graph);
                let values_for_input = Arc::clone(&values);
                let buffer_for_input = Arc::clone(&buffer);
                let graph_for_final = Arc::clone(&graph);
                let values_for_final = Arc::clone(&values);
                let buffer_for_final = Arc::clone(&buffer);
                let filter1_buffer = Arc::clone(&buffer);
                let filter1_values = Arc::clone(&values);
                let filter2_buffer = Arc::clone(&buffer);
                let filter2_values = Arc::clone(&values);
                let filter3_buffer = Arc::clone(&buffer);
                let filter3_values = Arc::clone(&values);
                let filter4_buffer = Arc::clone(&buffer);
                let filter4_values = Arc::clone(&values);
                let filter5_buffer = Arc::clone(&buffer);
                let filter5_values = Arc::clone(&values);
                let filter6_buffer = Arc::clone(&buffer);
                let filter6_values = Arc::clone(&values);

                let pipeline = Pipeline::from_pipes(
                    8,
                    vec![
                        Pipe::serial(move |ctx| {
                            if ctx.token() == total_nodes {
                                ctx.stop();
                                return;
                            }
                            let level = ctx.token() / graph_for_input.length;
                            let index = ctx.token() % graph_for_input.length;
                            let uid = graph_for_input.uid(level, index) as i32;
                            buffer_for_input[ctx.line()].store(uid, Ordering::Relaxed);
                            let value = graph_pipeline_work(uid);
                            values_for_input[uid as usize].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = filter1_buffer[ctx.line()].load(Ordering::Relaxed) as usize;
                            let value = graph_pipeline_work(uid as i32);
                            filter1_values[uid].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = filter2_buffer[ctx.line()].load(Ordering::Relaxed) as usize;
                            let value = graph_pipeline_work(uid as i32);
                            filter2_values[uid].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = filter3_buffer[ctx.line()].load(Ordering::Relaxed) as usize;
                            let value = graph_pipeline_work(uid as i32);
                            filter3_values[uid].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = filter4_buffer[ctx.line()].load(Ordering::Relaxed) as usize;
                            let value = graph_pipeline_work(uid as i32);
                            filter4_values[uid].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = filter5_buffer[ctx.line()].load(Ordering::Relaxed) as usize;
                            let value = graph_pipeline_work(uid as i32);
                            filter5_values[uid].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = filter6_buffer[ctx.line()].load(Ordering::Relaxed) as usize;
                            let value = graph_pipeline_work(uid as i32);
                            filter6_values[uid].store(value, Ordering::Relaxed);
                        }),
                        Pipe::serial(move |ctx| {
                            let uid = buffer_for_final[ctx.line()].load(Ordering::Relaxed) as usize;
                            let level = uid / graph_for_final.length;
                            let index = uid % graph_for_final.length;
                            let mut value = graph_pipeline_work(uid as i32);
                            if level != 0 {
                                let carry = graph_for_final.in_edges[level][index]
                                    .iter()
                                    .map(|(src, _)| {
                                        let previous = graph_for_final.uid(level - 1, *src);
                                        values_for_final[previous].load(Ordering::Relaxed)
                                    })
                                    .sum::<i32>();
                                value += carry;
                            }
                            values_for_final[uid].store(value, Ordering::Relaxed);
                        }),
                    ],
                );

                pipeline.run(&executor).wait().map_err(|error| error.to_string())?;
                let checksum = values
                    .iter()
                    .fold(0i64, |acc, value| acc + value.load(Ordering::Relaxed) as i64);
                Ok(checksum)
            })
        },
    )
}

fn run_graph_traversal_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = graph_traversal_points();
    let taskflow_driver = driver_graph_traversal(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "graph_traversal",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "graph_traversal",
        "Graph Traversal",
        Some("Report profile caps the graph dimension at 64 (original default: 2048).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |dimension| {
            let graph = LevelGraph::new(dimension, dimension);
            average_ms(config.rounds, || {
                let visited = Arc::new(
                    (0..dimension * dimension)
                        .map(|_| AtomicBool::new(false))
                        .collect::<Vec<_>>(),
                );
                let flow = Flow::new();
                let mut tasks = vec![Vec::with_capacity(dimension); dimension];
                for level in 0..dimension {
                    tasks[level] = Vec::with_capacity(dimension);
                }

                for index in 0..dimension {
                    let visited = Arc::clone(&visited);
                    let uid = graph.uid(dimension - 1, index);
                    tasks[dimension - 1].push(flow.spawn(move || {
                        let _ = bench_dummy(index);
                        visited[uid].store(true, Ordering::Relaxed);
                    }));
                }

                for level in (0..dimension.saturating_sub(1)).rev() {
                    for index in 0..dimension {
                        let visited = Arc::clone(&visited);
                        let uid = graph.uid(level, index);
                        let task = flow.spawn(move || {
                            let _ = bench_dummy(index);
                            visited[uid].store(true, Ordering::Relaxed);
                        });
                        for next in &graph.out_edges[level][index] {
                            task.precede([tasks[level + 1][*next].clone()]);
                        }
                        tasks[level].push(task);
                    }
                }

                executor
                    .run(&flow)
                    .wait()
                    .map_err(|error| error.to_string())?;
                let all_visited = visited.iter().all(|flag| flag.load(Ordering::Relaxed));
                if !all_visited {
                    return Err("graph traversal left unvisited nodes".to_string());
                }
                Ok(graph.graph_size())
            })
        },
    )
}

fn run_integrate_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(0, 500, 100);
    let taskflow_driver = driver_integrate(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "integrate",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "integrate",
        "Integrate",
        Some("Report profile caps the x-interval at 500 (original default: 2000).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |max_value| {
            average_ms(config.rounds, || {
                integrate_flow(
                    &executor,
                    0.0,
                    fn_integrate(0.0),
                    max_value as f64,
                    fn_integrate(max_value as f64),
                )
            })
        },
    )
}

fn run_linear_chain_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = pow2_points(1, 16);
    let taskflow_driver = driver_linear_chain(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "linear_chain",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "linear_chain",
        "Linear Chain",
        Some("Report profile caps the original Taskflow sweep at 2^16 tasks (original default: 2^25).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |length| {
            average_ms(config.rounds, || {
                let flow = Flow::new();
                let counter = Arc::new(AtomicUsize::new(0));
                let mut tasks = Vec::with_capacity(length);
                for _ in 0..length {
                    let counter = Arc::clone(&counter);
                    tasks.push(flow.spawn(move || {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }));
                }
                for index in 0..length.saturating_sub(1) {
                    tasks[index].precede([tasks[index + 1].clone()]);
                }
                executor.run(&flow).wait().map_err(|error| error.to_string())?;
                let count = counter.load(Ordering::Relaxed);
                if count != length {
                    return Err(format!("linear chain mismatch: expected {length}, got {count}"));
                }
                Ok(count)
            })
        },
    )
}

fn run_linear_pipeline_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = pow2_points(1, 10);
    let taskflow_driver = driver_linear_pipeline(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "linear_pipeline",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "linear_pipeline",
        "Linear Pipeline",
        Some("Taskflow defaults are preserved: pipes=`ssssssss`, num_lines=8. Report profile caps tokens at 2^10 (original default: 2^23).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let pipeline = Pipeline::from_pipes(
                    8,
                    vec![
                        Pipe::serial(move |ctx| {
                            linear_pipeline_work();
                            if ctx.token() == size {
                                ctx.stop();
                            }
                        }),
                        Pipe::serial(|_| linear_pipeline_work()),
                        Pipe::serial(|_| linear_pipeline_work()),
                        Pipe::serial(|_| linear_pipeline_work()),
                        Pipe::serial(|_| linear_pipeline_work()),
                        Pipe::serial(|_| linear_pipeline_work()),
                        Pipe::serial(|_| linear_pipeline_work()),
                        Pipe::serial(|_| linear_pipeline_work()),
                    ],
                );
                pipeline.run(&executor).wait().map_err(|error| error.to_string())?;
                Ok(pipeline.num_tokens())
            })
        },
    )
}

fn run_mandelbrot_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(100, 300, 100);
    let taskflow_driver = driver_mandelbrot(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "mandelbrot",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "mandelbrot",
        "Mandelbrot",
        Some(
            "Report profile caps the image size at 300x300 (original default: 1000x1000)."
                .to_string(),
        ),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let rows = Arc::<[usize]>::from((0..size).collect::<Vec<_>>());
                let output = parallel_transform(
                    &executor,
                    rows,
                    ParallelForOptions::default().with_chunk_size(1),
                    move |row| {
                        let mut buffer = vec![0u8; size * 3];
                        for column in 0..size {
                            let (x, y) = mandelbrot_scale(
                                *row as f64,
                                column as f64,
                                size as f64,
                                size as f64,
                            );
                            let value = mandelbrot_escape_time(x, y, 2);
                            let offset = column * 3;
                            let (r, g, b) = mandelbrot_color(value);
                            buffer[offset] = r;
                            buffer[offset + 1] = g;
                            buffer[offset + 2] = b;
                        }
                        buffer
                    },
                )
                .map_err(|error| error.to_string())?;
                let checksum = output.iter().fold(0u64, |acc, row| acc ^ checksum_u8(row));
                Ok(checksum)
            })
        },
    )
}

fn run_matrix_multiplication_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(128, 256, 64);
    let taskflow_driver = driver_matrix_multiplication(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "matrix_multiplication",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "matrix_multiplication",
        "Matrix Multiplication",
        Some("Report profile caps matrix size at 256 (original default: 1024).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let rows = Arc::<[usize]>::from((0..size).collect::<Vec<_>>());
                let output = parallel_transform(
                    &executor,
                    rows,
                    ParallelForOptions::default(),
                    move |row| {
                        let i = *row;
                        let mut values = vec![0f64; size];
                        for j in 0..size {
                            let mut sum = 0f64;
                            for k in 0..size {
                                let a = (i + k) as f64;
                                let b = (k * j) as f64;
                                sum += a * b;
                            }
                            values[j] = sum;
                        }
                        values
                    },
                )
                .map_err(|error| error.to_string())?;
                let checksum = output.iter().fold(0u64, |acc, row| acc ^ checksum_f64(row));
                Ok(checksum)
            })
        },
    )
}

fn run_merge_sort_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = decimal_points(5);
    let taskflow_driver = driver_merge_sort(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "merge_sort",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);
    c_rand_reset(1);

    compare_case(
        "merge_sort",
        "Merge Sort",
        Some("Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let mut values = vec![0f64; size];
                for value in &mut values {
                    *value = c_rand_value() as f64;
                }
                let output = merge_sort_flow(executor.clone(), values)?;
                Ok(checksum_f64(&output))
            })
        },
    )
}

fn run_nqueens_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(1, 10, 1);
    let taskflow_driver = driver_nqueens(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "nqueens",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "nqueens",
        "N-Queens",
        Some(
            "Report profile caps the original Taskflow sweep at 10 queens (original default: 14)."
                .to_string(),
        ),
        &points,
        taskflow_points,
        taskflow_note,
        |queens| {
            average_ms(config.rounds, || {
                let buffer = vec![0i8; queens];
                let value = nqueens_flow(&executor, 0, buffer)?;
                Ok(value)
            })
        },
    )
}

fn run_primes_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = decimal_points(5);
    let taskflow_driver = driver_primes(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "primes",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "primes",
        "Primes",
        Some(
            "Report profile caps the original Taskflow sweep at 10^5 (original default: 10^8)."
                .to_string(),
        ),
        &points,
        taskflow_points,
        taskflow_note,
        |limit| average_ms(config.rounds, || primes_flow(&executor, limit)),
    )
}

fn run_reduce_sum_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = decimal_points(6);
    let taskflow_driver = driver_reduce_sum(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "reduce_sum",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "reduce_sum",
        "Reduce Sum",
        Some("Report profile caps the original Taskflow sweep at 10^6 elements (original default: 10^9).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            let input = Arc::<[f64]>::from(vec![0.0; size]);
            average_ms(config.rounds, || {
                parallel_reduce(
                    &executor,
                    Arc::clone(&input),
                    ParallelForOptions::default(),
                    0.0f64,
                    |left, right| left + right,
                )
                .map_err(|error| error.to_string())
            })
        },
    )
}

fn run_scan_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = decimal_points(5);
    let taskflow_driver = driver_scan(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "scan",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);
    c_rand_reset(1);

    compare_case(
        "scan",
        "Scan",
        Some("Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let mut input = vec![0i32; size];
                for value in &mut input {
                    *value = c_rand_value();
                }
                let output = parallel_inclusive_scan(
                    &executor,
                    Arc::<[i32]>::from(input),
                    ParallelForOptions::default(),
                    |left, right| left.wrapping_mul(right),
                )
                .map_err(|error| error.to_string())?;
                Ok(checksum_i32(&output))
            })
        },
    )
}

fn run_skynet_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = stepped_points(1, 4, 1);
    let taskflow_driver = driver_skynet(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "skynet",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "skynet",
        "Skynet",
        Some(
            "Report profile caps the original Taskflow sweep at depth 4 (original default: 8)."
                .to_string(),
        ),
        &points,
        taskflow_points,
        taskflow_note,
        |depth| average_ms(config.rounds, || skynet_flow(&executor, 0, 0, depth)),
    )
}

fn run_sort_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = decimal_points(5);
    let taskflow_driver = driver_sort(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "sort",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);
    c_rand_reset(1);

    compare_case(
        "sort",
        "Sort",
        Some("Report profile caps the original Taskflow sweep at 10^5 elements (original default: 10^8).".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |size| {
            average_ms(config.rounds, || {
                let mut input = vec![0i32; size];
                for value in &mut input {
                    *value = c_rand_value();
                }
                let output = parallel_sort(
                    &executor,
                    input,
                    ParallelForOptions::default(),
                )
                .map_err(|error| error.to_string())?;
                Ok(checksum_i32(&output))
            })
        },
    )
}

fn run_thread_pool_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = vec![
        PointSpec {
            label: "100".to_string(),
            value: 100,
        },
        PointSpec {
            label: "50".to_string(),
            value: 50,
        },
        PointSpec {
            label: "20".to_string(),
            value: 20,
        },
        PointSpec {
            label: "10".to_string(),
            value: 10,
        },
        PointSpec {
            label: "5".to_string(),
            value: 5,
        },
    ];
    let taskflow_driver = driver_thread_pool(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "thread_pool",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "thread_pool",
        "Thread Pool",
        Some("Taskflow's original benchmark compares its executor against a custom thread pool. This report reuses the same Taskflow workload shape and compares RustFlow against Taskflow's executor-only path.".to_string()),
        &points,
        taskflow_points,
        taskflow_note,
        |iterations| average_ms(config.rounds, || thread_pool_flow(&executor, iterations)),
    )
}

fn run_wavefront_case(config: &Config, harness: &TaskflowHarness) -> CaseResult {
    let points = wavefront_points();
    let taskflow_driver = driver_wavefront(harness);
    let (taskflow_points, taskflow_note) = taskflow_map_note(harness.run_case(
        "wavefront",
        taskflow_driver,
        config.threads,
        config.rounds,
        &points,
    ));
    let executor = Executor::new(config.threads);

    compare_case(
        "wavefront",
        "Wavefront",
        Some(
            "Report profile caps the grid size at 256x256 blocks (original default: 16384x16384)."
                .to_string(),
        ),
        &points,
        taskflow_points,
        taskflow_note,
        |size| average_ms(config.rounds, || wavefront_flow(&executor, size)),
    )
}

type ValueSlot<T> = Arc<Mutex<Option<T>>>;

struct FlowValue<T> {
    task: TaskHandle,
    slot: ValueSlot<T>,
}

fn take_slot<T>(slot: &ValueSlot<T>) -> T {
    slot.lock()
        .expect("flow value slot poisoned")
        .take()
        .expect("flow value slot should be initialized")
}

fn fibonacci_runtime(runtime: &RuntimeCtx, n: usize) -> usize {
    if n < 2 {
        return n;
    }

    let mut left_value = [0usize; 1];
    let left_value_ptr = SharedMutPtr::from_slice(&mut left_value);
    runtime.silent_async(move |runtime| unsafe {
        left_value_ptr.write(0, fibonacci_runtime(runtime, n - 1));
    });
    let right = fibonacci_runtime(runtime, n - 2);
    runtime
        .corun_children()
        .expect("left fibonacci runtime async should succeed");

    left_value[0] + right
}

fn fibonacci_flow(executor: &Executor, n: usize) -> Result<usize, String> {
    let flow = Flow::new();
    let result = Arc::new(Mutex::new(None));
    {
        let result = Arc::clone(&result);
        flow.spawn_runtime(move |runtime| {
            *result.lock().expect("fibonacci slot poisoned") = Some(fibonacci_runtime(runtime, n));
        });
    }
    executor.run(&flow).wait().map_err(|error| error.to_string())?;
    Ok(result
        .lock()
        .expect("fibonacci slot poisoned")
        .take()
        .expect("fibonacci result should be initialized"))
}

fn fn_integrate(x: f64) -> f64 {
    (x * x + 1.0) * x
}

fn integrate_sequential_iterative(x1: f64, y1: f64, x2: f64, y2: f64, area: f64) -> f64 {
    const EPSILON: f64 = 1.0e-9;
    let mut stack = vec![(x1, y1, x2, y2, area)];
    let mut total = 0.0;

    while let Some((x1, y1, x2, y2, area)) = stack.pop() {
        let half = (x2 - x1) / 2.0;
        let x0 = x1 + half;
        let y0 = fn_integrate(x0);

        let area_x1x0 = (y1 + y0) / 2.0 * half;
        let area_x0x2 = (y0 + y2) / 2.0 * half;
        let area_x1x2 = area_x1x0 + area_x0x2;

        if (area_x1x2 - area).abs() < EPSILON {
            total += area_x1x2;
            continue;
        }

        stack.push((x1, y1, x0, y0, area_x1x0));
        stack.push((x0, y0, x2, y2, area_x0x2));
    }

    total
}

fn integrate_runtime(
    runtime: &RuntimeCtx,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    area: f64,
    depth: usize,
) -> f64 {
    const EPSILON: f64 = 1.0e-9;
    const RUNTIME_DEPTH_LIMIT: usize = 8;
    let half = (x2 - x1) / 2.0;
    let x0 = x1 + half;
    let y0 = fn_integrate(x0);

    let area_x1x0 = (y1 + y0) / 2.0 * half;
    let area_x0x2 = (y0 + y2) / 2.0 * half;
    let area_x1x2 = area_x1x0 + area_x0x2;

    if (area_x1x2 - area).abs() < EPSILON {
        return area_x1x2;
    }

    // Keep the outer recursive async shape aligned with Taskflow, but switch to an
    // explicit worklist before Rust call stacks become the bottleneck.
    if depth >= RUNTIME_DEPTH_LIMIT {
        return integrate_sequential_iterative(x1, y1, x2, y2, area);
    }

    let left = runtime.executor().runtime_async(move |runtime| {
        integrate_runtime(runtime, x1, y1, x0, y0, area_x1x0, depth + 1)
    });
    let right = integrate_runtime(runtime, x0, y0, x2, y2, area_x0x2, depth + 1);

    runtime
        .wait_async(left)
        .expect("left integrate runtime async should succeed")
        + right
}

fn integrate_flow(executor: &Executor, x1: f64, y1: f64, x2: f64, y2: f64) -> Result<f64, String> {
    let flow = Flow::new();
    let result = Arc::new(Mutex::new(None));
    {
        let result = Arc::clone(&result);
        flow.spawn_runtime(move |runtime| {
            *result.lock().expect("integrate slot poisoned") =
                Some(integrate_runtime(runtime, x1, y1, x2, y2, 0.0, 0));
        });
    }
    executor
        .run(&flow)
        .wait()
        .map_err(|error| error.to_string())?;
    Ok(result
        .lock()
        .expect("integrate slot poisoned")
        .take()
        .expect("integrate result should be initialized"))
}

fn merge_sorted(left: Vec<f64>, right: Vec<f64>) -> Vec<f64> {
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut merged = Vec::with_capacity(left.len() + right.len());

    while let (Some(left_value), Some(right_value)) = (left.peek(), right.peek()) {
        if left_value <= right_value {
            merged.push(left.next().expect("left should have a value"));
        } else {
            merged.push(right.next().expect("right should have a value"));
        }
    }

    merged.extend(left);
    merged.extend(right);
    merged
}

fn build_merge_sort(flow: &Flow, mut data: Vec<f64>) -> FlowValue<Vec<f64>> {
    let slot = Arc::new(Mutex::new(None));
    if data.len() <= 1_024 {
        data.sort_by(|left, right| left.partial_cmp(right).unwrap());
        let data_slot = Arc::new(Mutex::new(Some(data)));
        let data_slot_for_task = Arc::clone(&data_slot);
        let slot_for_task = Arc::clone(&slot);
        let task = flow.spawn(move || {
            let data = data_slot_for_task
                .lock()
                .expect("merge sort base data poisoned")
                .take()
                .expect("merge sort base data should exist");
            *slot_for_task.lock().expect("merge sort slot poisoned") = Some(data);
        });
        return FlowValue { task, slot };
    }

    let mid = data.len() / 2;
    let right = data.split_off(mid);
    let left = data;
    let left = build_merge_sort(flow, left);
    let right = build_merge_sort(flow, right);
    let left_slot = Arc::clone(&left.slot);
    let right_slot = Arc::clone(&right.slot);
    let slot_for_task = Arc::clone(&slot);
    let task = flow.spawn(move || {
        let merged = merge_sorted(take_slot(&left_slot), take_slot(&right_slot));
        *slot_for_task.lock().expect("merge sort slot poisoned") = Some(merged);
    });
    task.succeed([left.task.clone(), right.task.clone()]);
    FlowValue { task, slot }
}

fn merge_sort_flow(executor: Executor, data: Vec<f64>) -> Result<Vec<f64>, String> {
    let flow = Flow::new();
    let result = build_merge_sort(&flow, data);
    executor
        .run(&flow)
        .wait()
        .map_err(|error| error.to_string())?;
    Ok(take_slot(&result.slot))
}

fn queens_ok(a: &[i8], n: usize) -> bool {
    for i in 0..n {
        let p = a[i];
        for j in i + 1..n {
            let q = a[j];
            if q == p || q == p - (j - i) as i8 || q == p + (j - i) as i8 {
                return false;
            }
        }
    }
    true
}

fn nqueens_sequential(column: usize, buffer: &mut [i8]) -> i32 {
    let size = buffer.len();
    if size == column {
        return 1;
    }

    let mut total = 0i32;
    for row in 0..size {
        buffer[column] = row as i8;
        if queens_ok(buffer, column + 1) {
            total += nqueens_sequential(column + 1, buffer);
        }
    }
    total
}

fn nqueens_runtime(runtime: &RuntimeCtx, column: usize, buffer: Vec<i8>) -> i32 {
    const SEQUENTIAL_REMAINING: usize = 4;
    let size = buffer.len();
    if size == column {
        return 1;
    }
    if size - column <= SEQUENTIAL_REMAINING {
        let mut buffer = buffer;
        return nqueens_sequential(column, &mut buffer);
    }

    let mut parts = vec![0i32; size];
    let parts_ptr = SharedMutPtr::from_slice(parts.as_mut_slice());
    for row in 0..size {
        let mut next = buffer.clone();
        next[column] = row as i8;

        if queens_ok(&next, column + 1) {
            runtime.silent_async(move |runtime| unsafe {
                parts_ptr.write(row, nqueens_runtime(runtime, column + 1, next));
            });
        }
    }

    runtime
        .corun_children()
        .expect("nqueens runtime async children should succeed");

    parts.iter().copied().sum()
}

fn nqueens_flow(executor: &Executor, column: usize, buffer: Vec<i8>) -> Result<i32, String> {
    executor
        .runtime_async(move |runtime| nqueens_runtime(runtime, column, buffer))
        .wait()
        .map_err(|error| error.to_string())
}

fn is_prime(value: usize) -> bool {
    if value == 2 || value == 3 {
        return true;
    }
    if value <= 1 || value % 2 == 0 || value % 3 == 0 {
        return false;
    }
    let mut divisor = 5usize;
    while divisor * divisor <= value {
        if value % divisor == 0 || value % (divisor + 2) == 0 {
            return false;
        }
        divisor += 6;
    }
    true
}

fn primes_flow(executor: &Executor, limit: usize) -> Result<usize, String> {
    const CHUNK_SIZE: usize = 100; // Larger chunk size for dynamic partitioning
    if limit <= 1 {
        return Ok(0);
    }

    let flow = Flow::new();
    let workers = executor.num_workers();

    // Use dynamic partitioning for better load balancing
    let state = Arc::new(PartitionState::new(limit - 1)); // Check numbers from 1 to limit-1
    let partitioner = Arc::new(DynamicPartitioner::new(CHUNK_SIZE));
    let total_count = Arc::new(AtomicUsize::new(0));

    // Spawn worker tasks that dynamically claim chunks
    for _ in 0..workers {
        let state = Arc::clone(&state);
        let partitioner = Arc::clone(&partitioner);
        let total_count = Arc::clone(&total_count);

        flow.spawn(move || {
            // Dynamically claim chunks until exhausted
            while let Some(chunk) = partitioner.next_chunk(&state) {
                // Chunk indices are 0-based, but we check numbers from 1
                let start = chunk.start + 1;
                let end = chunk.end + 1;
                let mut local_count = 0usize;
                for value in start..end.min(limit) {
                    local_count += is_prime(value) as usize;
                }
                // Atomically add to global count
                total_count.fetch_add(local_count, Ordering::Relaxed);
            }
        });
    }

    executor
        .run(&flow)
        .wait()
        .map_err(|error| error.to_string())?;

    Ok(total_count.load(Ordering::Relaxed))
}

fn skynet_sequential(base: usize, depth: usize, max_depth: usize) -> usize {
    if depth == max_depth {
        return base;
    }

    let mut depth_offset = 1usize;
    for _ in 0..max_depth.saturating_sub(depth + 1) {
        depth_offset *= 10;
    }

    let mut total = 0usize;
    for index in 0..10usize {
        total += skynet_sequential(base + depth_offset * index, depth + 1, max_depth);
    }
    total
}

fn skynet_runtime(runtime: &RuntimeCtx, base: usize, depth: usize, max_depth: usize) -> usize {
    const SEQUENTIAL_REMAINING: usize = 2;
    if depth == max_depth {
        return base;
    }
    if max_depth.saturating_sub(depth) <= SEQUENTIAL_REMAINING {
        return skynet_sequential(base, depth, max_depth);
    }

    let mut depth_offset = 1usize;
    for _ in 0..max_depth.saturating_sub(depth + 1) {
        depth_offset *= 10;
    }

    let mut parts = [0usize; 10];
    let parts_ptr = SharedMutPtr::from_slice(&mut parts);
    for index in 0..10usize {
        runtime.silent_async(move |runtime| unsafe {
            parts_ptr.write(
                index,
                skynet_runtime(runtime, base + depth_offset * index, depth + 1, max_depth),
            );
        });
    }

    runtime
        .corun_children()
        .expect("skynet runtime async children should succeed");

    parts.iter().copied().sum()
}

fn skynet_flow(
    executor: &Executor,
    base: usize,
    depth: usize,
    max_depth: usize,
) -> Result<usize, String> {
    executor
        .runtime_async(move |runtime| skynet_runtime(runtime, base, depth, max_depth))
        .wait()
        .map_err(|error| error.to_string())
}

fn thread_pool_bench_func(loop_len: u64) -> f32 {
    let mut acc = 0f32;
    for value in 0..loop_len {
        acc += value as f32;
    }
    acc
}

fn thread_pool_flow(executor: &Executor, iterations: usize) -> Result<u64, String> {
    const NUM_BLOCKS: usize = 1_000;
    const LOOP_LEN: u64 = 100;
    let total = Arc::new(AtomicU64::new(0));

    for _ in 0..iterations {
        let flow = Flow::new();
        for _ in 0..NUM_BLOCKS {
            let total = Arc::clone(&total);
            flow.spawn(move || {
                total.fetch_add(
                    thread_pool_bench_func(LOOP_LEN).to_bits() as u64,
                    Ordering::Relaxed,
                );
            });
        }
        executor
            .run(&flow)
            .wait()
            .map_err(|error| error.to_string())?;
    }

    Ok(total.load(Ordering::Relaxed))
}

fn wavefront_flow(executor: &Executor, size: usize) -> Result<usize, String> {
    let m = size;
    let n = size;
    let b = 8usize;
    let mb = m.div_ceil(b);
    let nb = n.div_ceil(b);
    let flow = Flow::new();
    let total = Arc::new(AtomicUsize::new(0));
    let mut tasks = vec![Vec::with_capacity(nb); mb];
    for row in &mut tasks {
        for _ in 0..nb {
            row.push(flow.placeholder());
        }
    }

    for i in (0..mb).rev() {
        for j in (0..nb).rev() {
            let total = Arc::clone(&total);
            tasks[i][j] = flow.spawn(move || {
                total.fetch_add(i + j, Ordering::Relaxed);
            });
            if j + 1 < nb {
                tasks[i][j].precede([tasks[i][j + 1].clone()]);
            }
            if i + 1 < mb {
                tasks[i][j].precede([tasks[i + 1][j].clone()]);
            }
        }
    }

    executor
        .run(&flow)
        .wait()
        .map_err(|error| error.to_string())?;
    Ok(total.load(Ordering::Relaxed))
}

fn mandelbrot_scale(x: f64, y: f64, width: f64, height: f64) -> (f64, f64) {
    let xx = -2.5 + (1.0 - -2.5) / width * x;
    let yy = -1.0 + (1.0 - -1.0) / height * y;
    (xx, yy)
}

fn mandelbrot_escape_time(px: f64, py: f64, degree: i32) -> i32 {
    let mut x = px;
    let mut y = py;
    let mut x2 = x * x;
    let mut y2 = y * y;
    let cr = px;
    let ci = py;

    let mut iteration = 0i32;
    while (x2 + y2) <= 4.0 && iteration < 200 {
        let power = (x * x + y * y).powf(degree as f64 / 2.0);
        let angle = (degree as f64) * y.atan2(x);
        let xtmp = power * angle.cos() + cr;
        y = power * angle.sin() + ci;
        x = xtmp;
        x2 = x * x;
        y2 = y * y;
        iteration += 1;
    }
    iteration
}

fn mandelbrot_color(iteration: i32) -> (u8, u8, u8) {
    const COLORS: &[(u8, u8, u8)] = &[
        (66, 30, 15),
        (25, 7, 26),
        (9, 1, 47),
        (4, 4, 73),
        (0, 7, 100),
        (12, 44, 138),
        (24, 82, 177),
        (57, 125, 209),
        (134, 181, 229),
        (211, 236, 248),
        (241, 233, 191),
        (248, 201, 95),
        (255, 170, 0),
        (204, 128, 0),
        (153, 87, 0),
        (106, 52, 3),
    ];
    if iteration < 200 && iteration != 0 {
        COLORS[(iteration as usize) % 16]
    } else {
        (0, 0, 0)
    }
}

fn driver_async_task(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("async_task/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: async_task <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, size).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_binary_tree(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("binary_tree/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: binary_tree <threads> <rounds> <layers...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto layers = parse_size(argv[i]);
    auto label = size_t(1) << layers;
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(layers, threads).count();
    }
    print_point(label, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_black_scholes(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let header = harness.benchmark_cpp("black_scholes/common.hpp");
    let taskflow = harness.benchmark_cpp("black_scholes/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&header));
    source.push_str(
        r#"
OptionData *optdata = nullptr;
float *prices = nullptr;
int numOptions = 0;
int NUM_RUNS = 1;
int* otype = nullptr;
float* sptprice = nullptr;
float* strike = nullptr;
float* rate = nullptr;
float* volatility = nullptr;
float* otime = nullptr;
int numError = 0;
float* BUFFER = nullptr;
int* BUFFER2 = nullptr;
"#,
    );
    let _ = writeln!(source, "#include {}", cpp_string_literal(&taskflow));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: black_scholes <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    generate_options(size);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads).count();
    }
    destroy_options();
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_data_pipeline(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("data_pipeline/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: data_pipeline <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  std::string pipes = "ssssssss";
  unsigned num_lines = 8;
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(pipes, num_lines, threads, size).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_embarrassing_parallelism(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("embarrassing_parallelism/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: embarrassing_parallelism <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, size).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_fibonacci(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("fibonacci/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: fibonacci <threads> <rounds> <values...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto value = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, value).count();
    }
    print_point(value, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_for_each(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("for_each/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: for_each <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    vec.resize(size);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      for(auto& value : vec) {
        value = ::rand();
      }
      total += measure_time_taskflow(threads).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_graph_pipeline(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("graph_pipeline/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: graph_pipeline <threads> <rounds> <dimensions...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  unsigned num_lines = 8;
  size_t pipes = 8;
  for(int i = 3; i < argc; ++i) {
    auto dimension = parse_size(argv[i]);
    LevelGraph graph(dimension, dimension);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(graph, pipes, num_lines, threads).count();
      graph.clear_graph();
    }
    print_point(graph.graph_size(), total, rounds);
  }
}
"#,
    );
    source
}

fn driver_graph_traversal(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("graph_traversal/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: graph_traversal <threads> <rounds> <dimensions...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto dimension = parse_size(argv[i]);
    LevelGraph graph(dimension, dimension);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(graph, threads).count();
      graph.clear_graph();
    }
    print_point(graph.graph_size(), total, rounds);
  }
}
"#,
    );
    source
}

fn driver_integrate(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("integrate/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: integrate <threads> <rounds> <values...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto value = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, value).count();
    }
    print_point(value, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_linear_chain(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("linear_chain/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: linear_chain <threads> <rounds> <lengths...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto length = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(length, threads).count();
    }
    print_point(length, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_linear_pipeline(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("linear_pipeline/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: linear_pipeline <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  std::string pipes = "ssssssss";
  unsigned num_lines = 8;
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(pipes, num_lines, threads, size).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_mandelbrot(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let taskflow = harness.benchmark_cpp("mandelbrot/taskflow.cpp");
    source.push_str(
        r#"
int H = 0;
int W = 0;
unsigned char* RGB = nullptr;
"#,
    );
    let _ = writeln!(source, "#include {}", cpp_string_literal(&taskflow));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: mandelbrot <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = static_cast<int>(parse_size(argv[i]));
    W = size;
    H = size;
    RGB = static_cast<unsigned char*>(std::malloc(W * H * 3 * sizeof(unsigned char)));
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads).count();
    }
    std::free(RGB);
    RGB = nullptr;
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_matrix_multiplication(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let taskflow = harness.benchmark_cpp("matrix_multiplication/taskflow.cpp");
    source.push_str(
        r#"
int N = 0;
double **a = nullptr, **b = nullptr, **c = nullptr;
"#,
    );
    let _ = writeln!(source, "#include {}", cpp_string_literal(&taskflow));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: matrix_multiplication <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    N = static_cast<int>(parse_size(argv[i]));
    allocate_matrix();
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads).count();
    }
    deallocate_matrix();
    print_point(N, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_merge_sort(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("merge_sort/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: merge_sort <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    vec.resize(size);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      for(auto& value : vec) {
        value = ::rand();
      }
      total += measure_time_taskflow(threads).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_nqueens(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("nqueens/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: nqueens <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto queens = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, queens).count();
    }
    print_point(queens, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_primes(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("primes/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: primes <threads> <rounds> <limits...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto limit = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, limit).count();
    }
    print_point(limit, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_reduce_sum(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("reduce_sum/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: reduce_sum <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    vec.resize(size);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_scan(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("scan/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: scan <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    input.resize(size);
    output.resize(size);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      for(auto& value : input) {
        value = ::rand();
      }
      total += measure_time_taskflow(threads).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_skynet(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("skynet/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: skynet <threads> <rounds> <depths...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto depth = parse_size(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, depth).count();
    }
    print_point(depth, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_sort(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let include = harness.benchmark_cpp("sort/taskflow.cpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&include));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: sort <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = parse_size(argv[i]);
    vec.resize(size);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      for(auto& value : vec) {
        value = ::rand();
      }
      total += measure_time_taskflow(threads).count();
    }
    print_point(size, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_thread_pool(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let taskflow_header = harness.taskflow_root.join("taskflow/taskflow.hpp");
    let _ = writeln!(source, "#include {}", cpp_string_literal(&taskflow_header));
    source.push_str(
        r#"
float benchFunc(uint64_t loopLen) {
  float acc = 0;
  for(uint64_t k = 0; k < loopLen; ++k) {
    acc += k;
  }
  return acc;
}

double measure_time_taskflow(unsigned num_threads, unsigned iter) {
  constexpr uint64_t num_blocks = 1000;
  constexpr uint64_t loopLen = 100;
  tf::Executor executor(num_threads);
  auto beg = std::chrono::high_resolution_clock::now();
  for(uint64_t it = 0; it < iter; ++it) {
    tf::Taskflow taskflow;
    std::vector<tf::Task> nodes;
    nodes.reserve(num_blocks);
    for(uint64_t i = 0; i < num_blocks; ++i) {
      nodes.emplace_back(taskflow.emplace([=]() { benchFunc(loopLen); }));
    }
    executor.run(taskflow).wait();
  }
  auto end = std::chrono::high_resolution_clock::now();
  return std::chrono::duration_cast<std::chrono::microseconds>(end - beg).count();
}

int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: thread_pool <threads> <rounds> <iters...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto iter = parse_unsigned(argv[i]);
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads, iter);
    }
    print_point(iter, total, rounds);
  }
}
"#,
    );
    source
}

fn driver_wavefront(harness: &TaskflowHarness) -> String {
    let mut source = String::new();
    source.push_str(driver_prelude());
    let taskflow = harness.benchmark_cpp("wavefront/taskflow.cpp");
    source.push_str(
        r#"
int M = 0;
int N = 0;
int B = 0;
int MB = 0;
int NB = 0;
double **matrix = nullptr;
"#,
    );
    let _ = writeln!(source, "#include {}", cpp_string_literal(&taskflow));
    source.push_str(
        r#"
int main(int argc, char** argv) {
  if(argc < 4) {
    throw std::runtime_error("usage: wavefront <threads> <rounds> <sizes...>");
  }
  auto threads = parse_unsigned(argv[1]);
  auto rounds = parse_unsigned(argv[2]);
  for(int i = 3; i < argc; ++i) {
    auto size = static_cast<int>(parse_size(argv[i]));
    M = N = size;
    B = 8;
    MB = (M / B) + (M % B > 0);
    NB = (N / B) + (N % B > 0);
    init_matrix();
    double total = 0.0;
    for(unsigned round = 0; round < rounds; ++round) {
      total += measure_time_taskflow(threads).count();
    }
    destroy_matrix();
    print_point(static_cast<size_t>(MB * NB), total, rounds);
  }
}
"#,
    );
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_integral(a: f64, b: f64) -> f64 {
        let indefinite = |x: f64| 0.25 * x * x * (x * x + 2.0);
        indefinite(b) - indefinite(a)
    }

    fn assert_relative_error(value: f64, expected: f64) {
        let scale = expected.abs().max(1.0);
        let relative_error = (value - expected).abs() / scale;
        assert!(
            relative_error < 1.0e-9,
            "relative error too large: value={value}, expected={expected}, relative_error={relative_error}"
        );
    }

    #[test]
    fn integrate_iterative_matches_closed_form() {
        let value =
            integrate_sequential_iterative(0.0, fn_integrate(0.0), 500.0, fn_integrate(500.0), 0.0);
        assert_relative_error(value, exact_integral(0.0, 500.0));
    }

    #[test]
    fn integrate_runtime_handles_large_interval() {
        let executor = Executor::new(4);
        let value = integrate_flow(
            &executor,
            0.0,
            fn_integrate(0.0),
            500.0,
            fn_integrate(500.0),
        )
        .expect("integrate flow should succeed");
        assert_relative_error(value, exact_integral(0.0, 500.0));
    }

    #[test]
    fn nqueens_runtime_matches_known_answer() {
        let executor = Executor::new(4);
        let value = nqueens_flow(&executor, 0, vec![0i8; 10]).expect("nqueens flow should succeed");
        assert_eq!(value, 724);
    }

    #[test]
    fn skynet_runtime_matches_closed_form() {
        let executor = Executor::new(4);
        let depth = 4usize;
        let value = skynet_flow(&executor, 0, 0, depth).expect("skynet flow should succeed");
        let leaves = 10usize.pow(depth as u32);
        assert_eq!(value, leaves * (leaves - 1) / 2);
    }
}
