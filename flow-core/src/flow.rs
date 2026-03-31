use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use crate::runtime::RuntimeCtx;
use crate::semaphore::Semaphore;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct TaskId(usize);

impl TaskId {
    pub fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FlowTaskKind {
    Placeholder,
    Static,
    Runtime,
    Condition,
    MultiCondition,
    Subflow,
    Composed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowTaskInfo {
    id: TaskId,
    name: Option<String>,
    kind: FlowTaskKind,
    predecessors: Vec<TaskId>,
    successors: Vec<TaskId>,
}

impl FlowTaskInfo {
    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn kind(&self) -> FlowTaskKind {
        self.kind
    }

    pub fn predecessors(&self) -> &[TaskId] {
        &self.predecessors
    }

    pub fn successors(&self) -> &[TaskId] {
        &self.successors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowSummary {
    num_tasks: usize,
    num_edges: usize,
    num_roots: usize,
    num_leaves: usize,
}

impl FlowSummary {
    pub fn num_tasks(&self) -> usize {
        self.num_tasks
    }

    pub fn num_edges(&self) -> usize {
        self.num_edges
    }

    pub fn num_roots(&self) -> usize {
        self.num_roots
    }

    pub fn num_leaves(&self) -> usize {
        self.num_leaves
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FlowIssueKind {
    NoRoots,
    NonConditionalCycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowIssue {
    kind: FlowIssueKind,
    task_ids: Vec<TaskId>,
    message: String,
}

impl FlowIssue {
    pub fn kind(&self) -> FlowIssueKind {
        self.kind
    }

    pub fn task_ids(&self) -> &[TaskId] {
        &self.task_ids
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowInspection {
    summary: FlowSummary,
    roots: Vec<TaskId>,
    leaves: Vec<TaskId>,
    tasks: Vec<FlowTaskInfo>,
    issues: Vec<FlowIssue>,
}

impl FlowInspection {
    pub fn summary(&self) -> &FlowSummary {
        &self.summary
    }

    pub fn roots(&self) -> &[TaskId] {
        &self.roots
    }

    pub fn leaves(&self) -> &[TaskId] {
        &self.leaves
    }

    pub fn tasks(&self) -> &[FlowTaskInfo] {
        &self.tasks
    }

    pub fn task(&self, task_id: TaskId) -> Option<&FlowTaskInfo> {
        self.tasks.iter().find(|task| task.id == task_id)
    }

    pub fn issues(&self) -> &[FlowIssue] {
        &self.issues
    }

    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowValidationError {
    issues: Vec<FlowIssue>,
}

impl FlowValidationError {
    pub fn issues(&self) -> &[FlowIssue] {
        &self.issues
    }
}

impl fmt::Display for FlowValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.issues.is_empty() {
            return write!(f, "flow validation failed");
        }

        for (index, issue) in self.issues.iter().enumerate() {
            if index > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{}", issue.message)?;
        }

        Ok(())
    }
}

impl std::error::Error for FlowValidationError {}

type TaskFn = Arc<dyn Fn() + Send + Sync + 'static>;
type RuntimeFn = Arc<dyn Fn(&RuntimeCtx) + Send + Sync + 'static>;
type ConditionFn = Arc<dyn Fn() -> usize + Send + Sync + 'static>;
type MultiConditionFn = Arc<dyn Fn() -> Vec<usize> + Send + Sync + 'static>;
type SubflowFn = Arc<dyn Fn(&Subflow) + Send + Sync + 'static>;

#[derive(Clone)]
pub(crate) enum TaskKind {
    Placeholder,
    Static(TaskFn),
    Runtime(RuntimeFn),
    Condition(ConditionFn),
    MultiCondition(MultiConditionFn),
    Subflow(SubflowFn),
    Composed(Flow),
}

impl TaskKind {
    pub(crate) fn has_work(&self) -> bool {
        matches!(
            self,
            Self::Static(_)
                | Self::Runtime(_)
                | Self::Condition(_)
                | Self::MultiCondition(_)
                | Self::Subflow(_)
                | Self::Composed(_)
        )
    }

    pub(crate) fn is_condition(&self) -> bool {
        matches!(self, Self::Condition(_) | Self::MultiCondition(_))
    }
}

impl From<&TaskKind> for FlowTaskKind {
    fn from(kind: &TaskKind) -> Self {
        match kind {
            TaskKind::Placeholder => Self::Placeholder,
            TaskKind::Static(_) => Self::Static,
            TaskKind::Runtime(_) => Self::Runtime,
            TaskKind::Condition(_) => Self::Condition,
            TaskKind::MultiCondition(_) => Self::MultiCondition,
            TaskKind::Subflow(_) => Self::Subflow,
            TaskKind::Composed(_) => Self::Composed,
        }
    }
}

#[derive(Clone)]
pub(crate) struct NodeSnapshot {
    pub(crate) id: TaskId,
    pub(crate) name: Option<String>,
    pub(crate) kind: TaskKind,
    pub(crate) successors: Vec<usize>,
    pub(crate) total_predecessor_count: usize,
    pub(crate) predecessor_count: usize,
    pub(crate) to_acquire: Vec<Semaphore>,
    pub(crate) to_release: Vec<Semaphore>,
}

#[derive(Clone)]
pub(crate) struct GraphSnapshot {
    pub(crate) nodes: Vec<NodeSnapshot>,
}

struct Node {
    name: Option<String>,
    kind: TaskKind,
    successors: Vec<TaskId>,
    predecessors: Vec<TaskId>,
    to_acquire: Vec<Semaphore>,
    to_release: Vec<Semaphore>,
}

impl Node {
    fn new(kind: TaskKind) -> Self {
        Self {
            name: None,
            kind,
            successors: Vec::new(),
            predecessors: Vec::new(),
            to_acquire: Vec::new(),
            to_release: Vec::new(),
        }
    }
}

#[derive(Default)]
struct Graph {
    nodes: Vec<Node>,
}

fn add_edge(graph: &mut Graph, from: TaskId, to: TaskId) {
    if !graph.nodes[from.0].successors.contains(&to) {
        graph.nodes[from.0].successors.push(to);
    }
    if !graph.nodes[to.0].predecessors.contains(&from) {
        graph.nodes[to.0].predecessors.push(from);
    }
}

fn snapshot_graph(graph: &Graph) -> GraphSnapshot {
    let nodes = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| NodeSnapshot {
            id: TaskId(index),
            name: node.name.clone(),
            kind: node.kind.clone(),
            successors: node.successors.iter().map(|task_id| task_id.0).collect(),
            total_predecessor_count: node.predecessors.len(),
            predecessor_count: node
                .predecessors
                .iter()
                .filter(|task_id| !graph.nodes[task_id.0].kind.is_condition())
                .count(),
            to_acquire: node.to_acquire.clone(),
            to_release: node.to_release.clone(),
        })
        .collect();

    GraphSnapshot { nodes }
}

struct FlowInner {
    name: RwLock<String>,
    graph: RwLock<Graph>,
}

#[derive(Clone)]
pub struct Flow {
    inner: Arc<FlowInner>,
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

impl Flow {
    pub fn new() -> Self {
        Self::with_name("")
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(FlowInner {
                name: RwLock::new(name.into()),
                graph: RwLock::new(Graph::default()),
            }),
        }
    }

    pub fn builder(&self) -> FlowBuilder {
        FlowBuilder {
            flow: Arc::clone(&self.inner),
        }
    }

    pub fn spawn<F>(&self, task: F) -> TaskHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.builder().spawn(task)
    }

    pub fn spawn_condition<F>(&self, task: F) -> TaskHandle
    where
        F: Fn() -> usize + Send + Sync + 'static,
    {
        self.builder().spawn_condition(task)
    }

    pub fn spawn_multi_condition<F>(&self, task: F) -> TaskHandle
    where
        F: Fn() -> Vec<usize> + Send + Sync + 'static,
    {
        self.builder().spawn_multi_condition(task)
    }

    pub fn spawn_subflow<F>(&self, task: F) -> TaskHandle
    where
        F: Fn(&Subflow) + Send + Sync + 'static,
    {
        self.builder().spawn_subflow(task)
    }

    pub fn spawn_runtime<F>(&self, task: F) -> TaskHandle
    where
        F: Fn(&RuntimeCtx) + Send + Sync + 'static,
    {
        self.builder().spawn_runtime(task)
    }

    pub fn compose(&self, flow: &Flow) -> TaskHandle {
        self.builder().compose(flow)
    }

    pub fn placeholder(&self) -> TaskHandle {
        self.builder().placeholder()
    }

    pub fn name(&self) -> String {
        self.inner.name.read().expect("flow name poisoned").clone()
    }

    pub fn set_name(&self, name: impl Into<String>) {
        *self.inner.name.write().expect("flow name poisoned") = name.into();
    }

    pub fn num_tasks(&self) -> usize {
        self.inner
            .graph
            .read()
            .expect("flow graph poisoned")
            .nodes
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.num_tasks() == 0
    }

    pub fn dump_dot(&self) -> String {
        let graph = self.inner.graph.read().expect("flow graph poisoned");
        let flow_name = self.name();
        let mut output = String::from("digraph flow {\n");

        if !flow_name.is_empty() {
            output.push_str(&format!("  label=\"{}\";\n", escape_dot(&flow_name)));
        }

        for (index, node) in graph.nodes.iter().enumerate() {
            let label = node.name.as_deref().unwrap_or("");
            output.push_str(&format!("  {index} [label=\"{}\"];\n", escape_dot(label)));
            for successor in &node.successors {
                output.push_str(&format!("  {index} -> {};\n", successor.0));
            }
        }

        output.push_str("}\n");
        output
    }

    pub fn inspect(&self) -> FlowInspection {
        let graph = self.inner.graph.read().expect("flow graph poisoned");
        inspect_graph(&graph)
    }

    pub fn validate(&self) -> Result<(), FlowValidationError> {
        let inspection = self.inspect();
        if inspection.is_valid() {
            return Ok(());
        }

        Err(FlowValidationError {
            issues: inspection.issues,
        })
    }

    pub(crate) fn snapshot(&self) -> GraphSnapshot {
        let graph = self.inner.graph.read().expect("flow graph poisoned");
        snapshot_graph(&graph)
    }
}

#[derive(Clone)]
pub struct FlowBuilder {
    flow: Arc<FlowInner>,
}

impl FlowBuilder {
    pub fn spawn<F>(&self, task: F) -> TaskHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Static(Arc::new(task)))
    }

    pub fn placeholder(&self) -> TaskHandle {
        self.emplace(TaskKind::Placeholder)
    }

    pub fn spawn_runtime<F>(&self, task: F) -> TaskHandle
    where
        F: Fn(&RuntimeCtx) + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Runtime(Arc::new(task)))
    }

    pub fn spawn_condition<F>(&self, task: F) -> TaskHandle
    where
        F: Fn() -> usize + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Condition(Arc::new(task)))
    }

    pub fn spawn_multi_condition<F>(&self, task: F) -> TaskHandle
    where
        F: Fn() -> Vec<usize> + Send + Sync + 'static,
    {
        self.emplace(TaskKind::MultiCondition(Arc::new(task)))
    }

    pub fn spawn_subflow<F>(&self, task: F) -> TaskHandle
    where
        F: Fn(&Subflow) + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Subflow(Arc::new(task)))
    }

    pub fn compose(&self, flow: &Flow) -> TaskHandle {
        self.emplace(TaskKind::Composed(flow.clone()))
    }

    fn emplace(&self, kind: TaskKind) -> TaskHandle {
        let mut graph = self.flow.graph.write().expect("flow graph poisoned");
        let id = TaskId(graph.nodes.len());
        graph.nodes.push(Node::new(kind));
        TaskHandle {
            flow: Arc::clone(&self.flow),
            id,
        }
    }
}

#[derive(Clone)]
pub struct TaskHandle {
    flow: Arc<FlowInner>,
    id: TaskId,
}

impl PartialEq for TaskHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.flow, &other.flow) && self.id == other.id
    }
}

impl Eq for TaskHandle {}

impl TaskHandle {
    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn name(&self, name: impl Into<String>) -> Self {
        let mut graph = self.flow.graph.write().expect("flow graph poisoned");
        graph.nodes[self.id.0].name = Some(name.into());
        self.clone()
    }

    pub fn has_work(&self) -> bool {
        let graph = self.flow.graph.read().expect("flow graph poisoned");
        graph.nodes[self.id.0].kind.has_work()
    }

    pub fn acquire(&self, semaphore: &Semaphore) -> Self {
        let mut graph = self.flow.graph.write().expect("flow graph poisoned");
        graph.nodes[self.id.0].to_acquire.push(semaphore.clone());
        self.clone()
    }

    pub fn release(&self, semaphore: &Semaphore) -> Self {
        let mut graph = self.flow.graph.write().expect("flow graph poisoned");
        graph.nodes[self.id.0].to_release.push(semaphore.clone());
        self.clone()
    }

    pub fn precede<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = TaskHandle>,
    {
        let mut graph = self.flow.graph.write().expect("flow graph poisoned");
        for task in tasks {
            assert!(
                Arc::ptr_eq(&self.flow, &task.flow),
                "all tasks in a dependency relation must belong to the same flow"
            );
            add_edge(&mut graph, self.id, task.id);
        }
        self.clone()
    }

    pub fn succeed<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = TaskHandle>,
    {
        let predecessors: Vec<_> = tasks.into_iter().collect();
        for task in predecessors {
            task.precede([self.clone()]);
        }
        self.clone()
    }
}

fn escape_dot(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn inspect_graph(graph: &Graph) -> FlowInspection {
    let tasks: Vec<_> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| FlowTaskInfo {
            id: TaskId(index),
            name: node.name.clone(),
            kind: FlowTaskKind::from(&node.kind),
            predecessors: node.predecessors.clone(),
            successors: node.successors.clone(),
        })
        .collect();

    let roots: Vec<_> = tasks
        .iter()
        .filter(|task| task.predecessors.is_empty())
        .map(FlowTaskInfo::id)
        .collect();
    let leaves: Vec<_> = tasks
        .iter()
        .filter(|task| task.successors.is_empty())
        .map(FlowTaskInfo::id)
        .collect();

    let summary = FlowSummary {
        num_tasks: tasks.len(),
        num_edges: graph.nodes.iter().map(|node| node.successors.len()).sum(),
        num_roots: roots.len(),
        num_leaves: leaves.len(),
    };

    let mut issues = Vec::new();

    if !tasks.is_empty() && roots.is_empty() {
        issues.push(FlowIssue {
            kind: FlowIssueKind::NoRoots,
            task_ids: tasks.iter().map(FlowTaskInfo::id).collect(),
            message: "flow has no entry tasks".to_string(),
        });
    }

    let cycle_nodes = find_non_conditional_cycle_nodes(graph);
    if !cycle_nodes.is_empty() {
        issues.push(FlowIssue {
            kind: FlowIssueKind::NonConditionalCycle,
            task_ids: cycle_nodes,
            message: "flow contains a cycle formed by non-condition dependencies".to_string(),
        });
    }

    FlowInspection {
        summary,
        roots,
        leaves,
        tasks,
        issues,
    }
}

fn find_non_conditional_cycle_nodes(graph: &Graph) -> Vec<TaskId> {
    let mut indegree = vec![0usize; graph.nodes.len()];

    for node in &graph.nodes {
        if node.kind.is_condition() {
            continue;
        }

        for successor in &node.successors {
            indegree[successor.index()] += 1;
        }
    }

    let mut queue: VecDeque<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();

    while let Some(task_index) = queue.pop_front() {
        let node = &graph.nodes[task_index];
        if node.kind.is_condition() {
            continue;
        }

        for successor in &node.successors {
            let entry = &mut indegree[successor.index()];
            if *entry == 0 {
                continue;
            }
            *entry -= 1;
            if *entry == 0 {
                queue.push_back(successor.index());
            }
        }
    }

    indegree
        .into_iter()
        .enumerate()
        .filter_map(|(index, degree)| (degree > 0).then_some(TaskId(index)))
        .collect()
}

#[derive(Clone)]
pub struct Subflow {
    graph: Rc<RefCell<Graph>>,
}

impl Subflow {
    pub(crate) fn new() -> Self {
        Self {
            graph: Rc::new(RefCell::new(Graph::default())),
        }
    }

    pub fn spawn<F>(&self, task: F) -> SubflowTaskHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Static(Arc::new(task)))
    }

    pub fn placeholder(&self) -> SubflowTaskHandle {
        self.emplace(TaskKind::Placeholder)
    }

    pub fn spawn_runtime<F>(&self, task: F) -> SubflowTaskHandle
    where
        F: Fn(&RuntimeCtx) + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Runtime(Arc::new(task)))
    }

    pub fn spawn_condition<F>(&self, task: F) -> SubflowTaskHandle
    where
        F: Fn() -> usize + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Condition(Arc::new(task)))
    }

    pub fn spawn_multi_condition<F>(&self, task: F) -> SubflowTaskHandle
    where
        F: Fn() -> Vec<usize> + Send + Sync + 'static,
    {
        self.emplace(TaskKind::MultiCondition(Arc::new(task)))
    }

    pub fn spawn_subflow<F>(&self, task: F) -> SubflowTaskHandle
    where
        F: Fn(&Subflow) + Send + Sync + 'static,
    {
        self.emplace(TaskKind::Subflow(Arc::new(task)))
    }

    pub(crate) fn snapshot(&self) -> GraphSnapshot {
        let graph = self.graph.borrow();
        snapshot_graph(&graph)
    }

    fn emplace(&self, kind: TaskKind) -> SubflowTaskHandle {
        let mut graph = self.graph.borrow_mut();
        let id = TaskId(graph.nodes.len());
        graph.nodes.push(Node::new(kind));
        SubflowTaskHandle {
            graph: Rc::clone(&self.graph),
            id,
        }
    }
}

#[derive(Clone)]
pub struct SubflowTaskHandle {
    graph: Rc<RefCell<Graph>>,
    id: TaskId,
}

impl PartialEq for SubflowTaskHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.graph, &other.graph) && self.id == other.id
    }
}

impl Eq for SubflowTaskHandle {}

impl SubflowTaskHandle {
    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn name(&self, name: impl Into<String>) -> Self {
        let mut graph = self.graph.borrow_mut();
        graph.nodes[self.id.0].name = Some(name.into());
        self.clone()
    }

    pub fn has_work(&self) -> bool {
        let graph = self.graph.borrow();
        graph.nodes[self.id.0].kind.has_work()
    }

    pub fn acquire(&self, semaphore: &Semaphore) -> Self {
        let mut graph = self.graph.borrow_mut();
        graph.nodes[self.id.0].to_acquire.push(semaphore.clone());
        self.clone()
    }

    pub fn release(&self, semaphore: &Semaphore) -> Self {
        let mut graph = self.graph.borrow_mut();
        graph.nodes[self.id.0].to_release.push(semaphore.clone());
        self.clone()
    }

    pub fn precede<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = SubflowTaskHandle>,
    {
        let mut graph = self.graph.borrow_mut();
        for task in tasks {
            assert!(
                Rc::ptr_eq(&self.graph, &task.graph),
                "all tasks in a dependency relation must belong to the same subflow"
            );
            add_edge(&mut graph, self.id, task.id);
        }
        self.clone()
    }

    pub fn succeed<I>(&self, tasks: I) -> Self
    where
        I: IntoIterator<Item = SubflowTaskHandle>,
    {
        let predecessors: Vec<_> = tasks.into_iter().collect();
        for task in predecessors {
            task.precede([self.clone()]);
        }
        self.clone()
    }
}
