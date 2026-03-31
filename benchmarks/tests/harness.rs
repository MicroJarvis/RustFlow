use benchmarks::{BackendId, BenchmarkConfig, ScenarioId, run_benchmarks};

#[test]
fn benchmark_registry_exposes_required_backends() {
    let backends = BackendId::all();

    assert!(backends.contains(&BackendId::Flow));
    assert!(backends.contains(&BackendId::ThreadPool));
    assert!(backends.contains(&BackendId::Rayon));
    assert!(backends.contains(&BackendId::CppTaskflow));
}

#[test]
fn benchmark_registry_exposes_required_scenarios() {
    let scenarios = ScenarioId::all();

    assert!(scenarios.contains(&ScenarioId::Transform));
    assert!(scenarios.contains(&ScenarioId::Reduce));
    assert!(scenarios.contains(&ScenarioId::Find));
    assert!(scenarios.contains(&ScenarioId::InclusiveScan));
    assert!(scenarios.contains(&ScenarioId::Sort));
}

#[test]
fn benchmark_report_contains_regression_fields() {
    let report = run_benchmarks(
        BenchmarkConfig::for_test()
            .with_backends(vec![BackendId::Flow])
            .with_scenarios(vec![ScenarioId::Reduce]),
    )
    .expect("benchmark harness should run");

    let json = report.to_json();
    assert!(json.contains("\"median_ns\""));
    assert!(json.contains("\"samples_ns\""));
    assert!(json.contains("\"checksum\""));
}

#[test]
fn benchmark_harness_runs_internal_backends() {
    let report = run_benchmarks(
        BenchmarkConfig::for_test()
            .with_backends(vec![
                BackendId::Flow,
                BackendId::ThreadPool,
                BackendId::Rayon,
            ])
            .with_scenarios(vec![ScenarioId::Transform, ScenarioId::Sort]),
    )
    .expect("benchmark harness should run");

    let statuses = report.statuses();
    assert!(
        statuses
            .iter()
            .all(|status| status.backend != BackendId::CppTaskflow)
    );
    assert!(statuses.iter().all(|status| status.available));
}
