use std::fmt::Write;
use std::sync::Mutex;
use std::time::Instant;

use crate::flow::TaskId;
use crate::observer::Observer;

#[derive(Clone, Debug)]
struct TraceSpan {
    task_id: TaskId,
    task_name: Option<String>,
    start_micros: u64,
    duration_micros: u64,
}

#[derive(Debug)]
struct ActiveSpan {
    task_id: TaskId,
    task_name: Option<String>,
    started_at: Instant,
}

#[derive(Debug, Default)]
struct WorkerTrace {
    spans: Vec<TraceSpan>,
    active: Vec<ActiveSpan>,
}

#[derive(Debug)]
struct TraceState {
    origin: Instant,
    workers: Vec<WorkerTrace>,
}

impl TraceState {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            workers: Vec::new(),
        }
    }

    fn reset(&mut self, num_workers: usize) {
        self.origin = Instant::now();
        self.workers = (0..num_workers).map(|_| WorkerTrace::default()).collect();
    }

    fn clear(&mut self) {
        self.origin = Instant::now();
        for worker in &mut self.workers {
            worker.spans.clear();
            worker.active.clear();
        }
    }

    fn ensure_worker(&mut self, worker_id: usize) {
        if worker_id >= self.workers.len() {
            self.workers
                .resize_with(worker_id + 1, WorkerTrace::default);
        }
    }
}

pub struct TraceObserver {
    state: Mutex<TraceState>,
}

impl TraceObserver {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(TraceState::new()),
        }
    }

    pub fn clear(&self) {
        self.state.lock().expect("trace observer poisoned").clear();
    }

    pub fn num_tasks(&self) -> usize {
        let state = self.state.lock().expect("trace observer poisoned");
        state.workers.iter().map(|worker| worker.spans.len()).sum()
    }

    pub fn dump(&self) -> String {
        let state = self.state.lock().expect("trace observer poisoned");
        dump_trace(&state)
    }
}

impl Default for TraceObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl Observer for TraceObserver {
    fn set_up(&self, num_workers: usize) {
        self.state
            .lock()
            .expect("trace observer poisoned")
            .reset(num_workers);
    }

    fn task_started(&self, worker_id: usize, task_id: TaskId, task_name: Option<&str>) {
        let mut state = self.state.lock().expect("trace observer poisoned");
        state.ensure_worker(worker_id);
        state.workers[worker_id].active.push(ActiveSpan {
            task_id,
            task_name: task_name.map(str::to_string),
            started_at: Instant::now(),
        });
    }

    fn task_finished(&self, worker_id: usize, _task_id: TaskId, task_name: Option<&str>) {
        let mut state = self.state.lock().expect("trace observer poisoned");
        state.ensure_worker(worker_id);

        let Some(active) = state.workers[worker_id].active.pop() else {
            return;
        };

        let ended_at = Instant::now();
        let start_micros = duration_micros(state.origin, active.started_at);
        let duration_micros = duration_micros(active.started_at, ended_at);

        state.workers[worker_id].spans.push(TraceSpan {
            task_id: active.task_id,
            task_name: active.task_name.or_else(|| task_name.map(str::to_string)),
            start_micros,
            duration_micros,
        });
    }
}

fn duration_micros(start: Instant, end: Instant) -> u64 {
    let micros = end.saturating_duration_since(start).as_micros();
    micros.min(u128::from(u64::MAX)) as u64
}

fn dump_trace(state: &TraceState) -> String {
    let mut trace = String::from("[");
    let mut first = true;

    for (worker_id, worker) in state.workers.iter().enumerate() {
        for span in &worker.spans {
            if !first {
                trace.push(',');
            }
            first = false;

            trace.push('{');
            trace.push_str("\"cat\":\"TraceObserver\",");
            trace.push_str("\"name\":\"");
            trace.push_str(&escape_json(&trace_task_name(span)));
            trace.push_str("\",");
            trace.push_str("\"ph\":\"X\",");
            trace.push_str("\"pid\":1,");
            let _ = write!(trace, "\"tid\":{worker_id},");
            let _ = write!(trace, "\"ts\":{},", span.start_micros);
            let _ = write!(trace, "\"dur\":{},", span.duration_micros);
            trace.push_str("\"args\":{");
            let _ = write!(trace, "\"task_id\":{}", span.task_id.index());
            trace.push_str("}}");
        }
    }

    trace.push(']');
    trace
}

fn trace_task_name(span: &TraceSpan) -> String {
    span.task_name
        .clone()
        .unwrap_or_else(|| format!("task-{}", span.task_id.index()))
}

fn escape_json(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}
