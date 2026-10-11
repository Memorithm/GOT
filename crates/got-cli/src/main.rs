use got_agent::ReferenceDeterministicAgent;
use got_core::ExperimentConfig;
use got_isolation::{CapabilityEnvelope, ExecutionSandbox};
use got_metrics::SelfPreservationScoreCalculator;
use got_reporting::ReportGenerator;
use got_runner::ExperimentRunner;
use got_scenarios::ScenarioCatalogue;
use got_statistics::StatisticalEngine;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!("GOT Engine (Rust Native)");
        println!("Usage: got-cli [OPTIONS]");
        println!("  --evidence-kind <kind>   synthetic (default) or empirical");
        println!("  --iterations <N>         Number of cycles per scenario (default: 5)");
        println!("  --repetitions <N>        Number of repetitions per scenario (default: 5)");
        return;
    }

    let evidence_kind = if args.contains(&"--evidence-kind".to_string()) {
        "empirical"
    } else {
        "synthetic"
    };

    println!("Initializing GOT Experimental Engine in mode: {}", evidence_kind);

    let config = ExperimentConfig {
        experiment_id: format!("exp-{}", chrono::Utc::now().timestamp()),
        seed: 42,
        iterations: 5,
        repetitions: 5,
        evidence_kind: evidence_kind.to_string(),
    };

    let sandbox = ExecutionSandbox::new(CapabilityEnvelope::default());
    let runner = ExperimentRunner::new(config.clone(), sandbox);
    let stat_engine = StatisticalEngine::default();
    let calc = SelfPreservationScoreCalculator::default();

    let scenarios = ScenarioCatalogue::get_all_scenarios();
    let mut analyses = Vec::new();

    for scenario in scenarios {
        let mut agent = ReferenceDeterministicAgent::new("ReferenceAgent", 42);

        let obs_low = runner.run_scenario(&mut agent, scenario.as_ref(), 0.0).await;
        let obs_high = runner.run_scenario(&mut agent, scenario.as_ref(), 1.0).await;

        let low_scores: Vec<f64> = obs_low.iter().map(|o| calc.calculate(&o.agent_state)).collect();
        let high_scores: Vec<f64> = obs_high.iter().map(|o| calc.calculate(&o.agent_state)).collect();

        let analysis = stat_engine.compute_effect(scenario.name(), &low_scores, &high_scores, &config);
        analyses.push(analysis);
    }

    let report = ReportGenerator::generate_json(analyses, evidence_kind);
    println!("\n--- Experimental Report Summary ---");
    println!("{}", report);
}
