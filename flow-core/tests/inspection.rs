use flow_core::{Flow, FlowIssueKind, FlowTaskKind};

#[test]
fn flow_inspection_reports_summary_and_dependencies() {
    let flow = Flow::with_name("inspect");
    let a = flow.spawn(|| {}).name("A");
    let b = flow.spawn_condition(|| 0).name("B");
    let c = flow.placeholder().name("C");

    a.precede([b.clone()]);
    b.precede([c.clone()]);

    let inspection = flow.inspect();
    let summary = inspection.summary();

    assert!(inspection.is_valid());
    assert_eq!(summary.num_tasks(), 3);
    assert_eq!(summary.num_edges(), 2);
    assert_eq!(summary.num_roots(), 1);
    assert_eq!(summary.num_leaves(), 1);
    assert_eq!(inspection.roots(), &[a.id()]);
    assert_eq!(inspection.leaves(), &[c.id()]);

    let task = inspection.task(b.id()).expect("task B should exist");
    assert_eq!(task.name(), Some("B"));
    assert_eq!(task.kind(), FlowTaskKind::Condition);
    assert_eq!(task.predecessors(), &[a.id()]);
    assert_eq!(task.successors(), &[c.id()]);
}

#[test]
fn validation_allows_condition_feedback_loops_with_entry_tasks() {
    let flow = Flow::new();
    let init = flow.spawn(|| {}).name("init");
    let condition = flow.spawn_condition(|| 0).name("loop");
    let done = flow.spawn(|| {}).name("done");

    init.precede([condition.clone()]);
    condition.precede([condition.clone(), done]);

    let inspection = flow.inspect();

    assert!(inspection.is_valid());
    flow.validate()
        .expect("condition feedback loop with an entry task should be valid");
}

#[test]
fn validation_reports_non_conditional_cycles() {
    let flow = Flow::new();
    let init = flow.spawn(|| {}).name("init");
    let a = flow.spawn(|| {}).name("A");
    let b = flow.spawn(|| {}).name("B");

    init.precede([a.clone()]);
    a.precede([b.clone()]);
    b.precede([a.clone()]);

    let error = flow
        .validate()
        .expect_err("non-conditional cycle should be rejected");

    assert!(
        error
            .issues()
            .iter()
            .any(|issue| issue.kind() == FlowIssueKind::NonConditionalCycle)
    );
    let cycle = error
        .issues()
        .iter()
        .find(|issue| issue.kind() == FlowIssueKind::NonConditionalCycle)
        .expect("cycle issue should exist");
    assert!(cycle.task_ids().contains(&a.id()));
    assert!(cycle.task_ids().contains(&b.id()));
}

#[test]
fn validation_reports_missing_entry_tasks() {
    let flow = Flow::new();
    let a = flow.spawn(|| {}).name("A");
    let b = flow.spawn_condition(|| 0).name("B");

    a.precede([b.clone()]);
    b.precede([a.clone()]);

    let error = flow
        .validate()
        .expect_err("a flow without roots should be rejected");

    assert!(
        error
            .issues()
            .iter()
            .any(|issue| issue.kind() == FlowIssueKind::NoRoots)
    );
}
