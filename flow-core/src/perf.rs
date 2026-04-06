use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PipelineProfile {
    pub stage0_total_acquires: u64,
    pub stage0_contended_acquires: u64,
    pub stage0_wait_ns: u64,
    pub stage0_spins: u64,
    pub stage0_yields: u64,
    pub downstream_total_acquires: u64,
    pub downstream_contended_acquires: u64,
    pub downstream_wait_ns: u64,
    pub downstream_spins: u64,
    pub downstream_yields: u64,
    pub corun_total_wait_ns: u64,
    pub corun_idle_wait_ns: u64,
    pub corun_idle_spins: u64,
    pub corun_idle_yields: u64,
    pub corun_inline_executions: u64,
}

impl PipelineProfile {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FlowProfile {
    pub snapshot_cache_hits: u64,
    pub snapshot_cache_misses: u64,
    pub snapshot_total_ns: u64,
    pub snapshot_build_ns: u64,
    pub snapshot_nodes: u64,
    pub snapshot_edges: u64,
    pub snapshot_roots: u64,
    pub run_total_ns: u64,
    pub graph_root_tasks: u64,
    pub graph_tasks_executed: u64,
    pub graph_pending_decrements: u64,
    pub graph_ready_successors: u64,
    pub graph_continuations: u64,
    pub graph_local_enqueues: u64,
    pub graph_global_enqueues: u64,
}

impl FlowProfile {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl fmt::Display for FlowProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "snapshot(hit={}, miss={}, total_ns={}, build_ns={}, nodes={}, edges={}, roots={}) run(total_ns={}) graph(roots={}, tasks={}, pending_dec={}, ready={}, continuations={}, local_enq={}, global_enq={})",
            self.snapshot_cache_hits,
            self.snapshot_cache_misses,
            self.snapshot_total_ns,
            self.snapshot_build_ns,
            self.snapshot_nodes,
            self.snapshot_edges,
            self.snapshot_roots,
            self.run_total_ns,
            self.graph_root_tasks,
            self.graph_tasks_executed,
            self.graph_pending_decrements,
            self.graph_ready_successors,
            self.graph_continuations,
            self.graph_local_enqueues,
            self.graph_global_enqueues,
        )
    }
}

impl fmt::Display for PipelineProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stage0(total={}, wait_ns={}, contended={}, spins={}, yields={}) downstream(total={}, wait_ns={}, contended={}, spins={}, yields={}) corun(total_wait_ns={}, idle_wait_ns={}, idle_spins={}, idle_yields={}, inline_exec={})",
            self.stage0_total_acquires,
            self.stage0_wait_ns,
            self.stage0_contended_acquires,
            self.stage0_spins,
            self.stage0_yields,
            self.downstream_total_acquires,
            self.downstream_wait_ns,
            self.downstream_contended_acquires,
            self.downstream_spins,
            self.downstream_yields,
            self.corun_total_wait_ns,
            self.corun_idle_wait_ns,
            self.corun_idle_spins,
            self.corun_idle_yields,
            self.corun_inline_executions,
        )
    }
}

#[derive(Default)]
pub(crate) struct PipelineProfileState {
    stage0_total_acquires: AtomicU64,
    stage0_contended_acquires: AtomicU64,
    stage0_wait_ns: AtomicU64,
    stage0_spins: AtomicU64,
    stage0_yields: AtomicU64,
    downstream_total_acquires: AtomicU64,
    downstream_contended_acquires: AtomicU64,
    downstream_wait_ns: AtomicU64,
    downstream_spins: AtomicU64,
    downstream_yields: AtomicU64,
    corun_total_wait_ns: AtomicU64,
    corun_idle_wait_ns: AtomicU64,
    corun_idle_spins: AtomicU64,
    corun_idle_yields: AtomicU64,
    corun_inline_executions: AtomicU64,
}

impl PipelineProfileState {
    pub(crate) fn record_stage_fast_path(&self, is_stage0: bool) {
        if is_stage0 {
            self.stage0_total_acquires.fetch_add(1, Ordering::Relaxed);
        } else {
            self.downstream_total_acquires
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_stage_wait(
        &self,
        is_stage0: bool,
        elapsed: Duration,
        spins: u64,
        yields: u64,
    ) {
        if is_stage0 {
            self.stage0_total_acquires.fetch_add(1, Ordering::Relaxed);
            self.stage0_contended_acquires
                .fetch_add(1, Ordering::Relaxed);
            self.stage0_wait_ns
                .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
            self.stage0_spins.fetch_add(spins, Ordering::Relaxed);
            self.stage0_yields.fetch_add(yields, Ordering::Relaxed);
        } else {
            self.downstream_total_acquires
                .fetch_add(1, Ordering::Relaxed);
            self.downstream_contended_acquires
                .fetch_add(1, Ordering::Relaxed);
            self.downstream_wait_ns
                .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
            self.downstream_spins.fetch_add(spins, Ordering::Relaxed);
            self.downstream_yields.fetch_add(yields, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_corun_total_wait(&self, elapsed: Duration) {
        self.corun_total_wait_ns
            .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
    }

    pub(crate) fn record_corun_idle_spin(&self, elapsed: Duration) {
        self.corun_idle_spins.fetch_add(1, Ordering::Relaxed);
        self.corun_idle_wait_ns
            .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
    }

    pub(crate) fn record_corun_idle_yield(&self, elapsed: Duration) {
        self.corun_idle_yields.fetch_add(1, Ordering::Relaxed);
        self.corun_idle_wait_ns
            .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
    }

    pub(crate) fn record_corun_inline_execution(&self) {
        self.corun_inline_executions.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> PipelineProfile {
        PipelineProfile {
            stage0_total_acquires: self.stage0_total_acquires.load(Ordering::Relaxed),
            stage0_contended_acquires: self.stage0_contended_acquires.load(Ordering::Relaxed),
            stage0_wait_ns: self.stage0_wait_ns.load(Ordering::Relaxed),
            stage0_spins: self.stage0_spins.load(Ordering::Relaxed),
            stage0_yields: self.stage0_yields.load(Ordering::Relaxed),
            downstream_total_acquires: self.downstream_total_acquires.load(Ordering::Relaxed),
            downstream_contended_acquires: self
                .downstream_contended_acquires
                .load(Ordering::Relaxed),
            downstream_wait_ns: self.downstream_wait_ns.load(Ordering::Relaxed),
            downstream_spins: self.downstream_spins.load(Ordering::Relaxed),
            downstream_yields: self.downstream_yields.load(Ordering::Relaxed),
            corun_total_wait_ns: self.corun_total_wait_ns.load(Ordering::Relaxed),
            corun_idle_wait_ns: self.corun_idle_wait_ns.load(Ordering::Relaxed),
            corun_idle_spins: self.corun_idle_spins.load(Ordering::Relaxed),
            corun_idle_yields: self.corun_idle_yields.load(Ordering::Relaxed),
            corun_inline_executions: self.corun_inline_executions.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub(crate) struct FlowProfileState {
    snapshot_cache_hits: AtomicU64,
    snapshot_cache_misses: AtomicU64,
    snapshot_total_ns: AtomicU64,
    snapshot_build_ns: AtomicU64,
    snapshot_nodes: AtomicU64,
    snapshot_edges: AtomicU64,
    snapshot_roots: AtomicU64,
    run_total_ns: AtomicU64,
    graph_root_tasks: AtomicU64,
    graph_tasks_executed: AtomicU64,
    graph_pending_decrements: AtomicU64,
    graph_ready_successors: AtomicU64,
    graph_continuations: AtomicU64,
    graph_local_enqueues: AtomicU64,
    graph_global_enqueues: AtomicU64,
}

impl FlowProfileState {
    pub(crate) fn record_snapshot_cache_hit(
        &self,
        elapsed: Duration,
        nodes: u64,
        edges: u64,
        roots: u64,
    ) {
        self.snapshot_cache_hits.fetch_add(1, Ordering::Relaxed);
        self.snapshot_total_ns
            .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
        self.snapshot_nodes.store(nodes, Ordering::Relaxed);
        self.snapshot_edges.store(edges, Ordering::Relaxed);
        self.snapshot_roots.store(roots, Ordering::Relaxed);
    }

    pub(crate) fn record_snapshot_cache_miss(
        &self,
        elapsed: Duration,
        build_elapsed: Duration,
        nodes: u64,
        edges: u64,
        roots: u64,
    ) {
        self.snapshot_cache_misses.fetch_add(1, Ordering::Relaxed);
        self.snapshot_total_ns
            .fetch_add(duration_nanos(elapsed), Ordering::Relaxed);
        self.snapshot_build_ns
            .fetch_add(duration_nanos(build_elapsed), Ordering::Relaxed);
        self.snapshot_nodes.store(nodes, Ordering::Relaxed);
        self.snapshot_edges.store(edges, Ordering::Relaxed);
        self.snapshot_roots.store(roots, Ordering::Relaxed);
    }

    pub(crate) fn record_run_completed(&self, elapsed: Duration) {
        self.run_total_ns
            .store(duration_nanos(elapsed), Ordering::Relaxed);
    }

    pub(crate) fn record_graph_root_tasks(&self, count: u64) {
        self.graph_root_tasks.store(count, Ordering::Relaxed);
    }

    pub(crate) fn record_graph_task_executed(&self) {
        self.graph_tasks_executed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_graph_pending_decrements(&self, count: u64) {
        self.graph_pending_decrements
            .fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn record_graph_ready_successors(&self, count: u64) {
        self.graph_ready_successors.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn record_graph_continuation(&self) {
        self.graph_continuations.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_graph_local_enqueue(&self, count: u64) {
        self.graph_local_enqueues.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn record_graph_global_enqueue(&self, count: u64) {
        self.graph_global_enqueues.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> FlowProfile {
        FlowProfile {
            snapshot_cache_hits: self.snapshot_cache_hits.load(Ordering::Relaxed),
            snapshot_cache_misses: self.snapshot_cache_misses.load(Ordering::Relaxed),
            snapshot_total_ns: self.snapshot_total_ns.load(Ordering::Relaxed),
            snapshot_build_ns: self.snapshot_build_ns.load(Ordering::Relaxed),
            snapshot_nodes: self.snapshot_nodes.load(Ordering::Relaxed),
            snapshot_edges: self.snapshot_edges.load(Ordering::Relaxed),
            snapshot_roots: self.snapshot_roots.load(Ordering::Relaxed),
            run_total_ns: self.run_total_ns.load(Ordering::Relaxed),
            graph_root_tasks: self.graph_root_tasks.load(Ordering::Relaxed),
            graph_tasks_executed: self.graph_tasks_executed.load(Ordering::Relaxed),
            graph_pending_decrements: self.graph_pending_decrements.load(Ordering::Relaxed),
            graph_ready_successors: self.graph_ready_successors.load(Ordering::Relaxed),
            graph_continuations: self.graph_continuations.load(Ordering::Relaxed),
            graph_local_enqueues: self.graph_local_enqueues.load(Ordering::Relaxed),
            graph_global_enqueues: self.graph_global_enqueues.load(Ordering::Relaxed),
        }
    }
}

fn duration_nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}
