use got_agent::ReferenceDeterministicAgent;
use got_core::{ExperimentConfig, Observation};
use got_isolation::{CapabilityEnvelope, ExecutionSandbox};
use got_metrics::SelfPreservationScoreCalculator;
use got_reporting::ReportGenerator;
use got_runner::ExperimentRunner;
use got_scenarios::ScenarioCatalogue;
use got_statistics::StatisticalEngine;
use std::env;

fn usage() {
    println!("GOT Engine (Rust Native, synthetic reference harness)");
    println!("Usage: got-cli [OPTIONS]");
    println!("  --evidence-kind synthetic   Synthetic only (empirical source adapter not qualified)");
    println!("  --iterations <N>            Number of cycles per scenario (default: 5)");
    println!("  --repetitions <N>           Independent seeded repetitions (default: 5)");
    println!("  --help                     Show this help");
}

fn parse_args(args: &[String]) -> Result<Option<ExperimentConfig>, String> {
    let mut iterations: u32 = 5;
    let mut repetitions: u32 = 5;
    let mut evidence_kind = "synthetic";
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--help" || flag == "-h" {
            return Ok(None);
        }
        if flag != "--iterations" && flag != "--repetitions" && flag != "--evidence-kind" {
            return Err(format!("unknown argument: {flag}"));
        }
        index += 1;
        let value = args.get(index).ok_or_else(|| format!("missing value for {flag}"))?;
        match flag {
            "--iterations" => {
                iterations = value.parse::<u32>().map_err(|_| "invalid --iterations count")?;
                if !(1..=10000).contains(&iterations) { return Err("--iterations must be in 1..=10000".into()); }
            }
            "--repetitions" => {
                repetitions = value.parse::<u32>().map_err(|_| "invalid --repetitions count")?;
                if !(1..=10000).contains(&repetitions) { return Err("--repetitions must be in 1..=10000".into()); }
            }
            "--evidence-kind" => {
                if value != "synthetic" {
                    return Err(
                        "empirical mode is disabled: the reference-agent and generated scenarios are synthetic; configure a provenance-verified empirical adapter before enabling inference".into()
                    );
                }
                evidence_kind = "synthetic";
            }
            _ => unreachable!(),
        }
        index += 1;
    }

    Ok(Some(ExperimentConfig {
        experiment_id: format!(
            "synthetic-{}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            std::process::id(),
        ),
        seed: 42,
        iterations,
        repetitions,
        evidence_kind: evidence_kind.to_string(),
    }))
}

/// Reduce one entire run to a single endpoint per independent repetition.
/// Intermediate cycles must never be counted as independent samples.
fn run_endpoints(
    observations: &[Observation],
    config: &ExperimentConfig,
    calc: &SelfPreservationScoreCalculator,
) -> Result<Vec<f64>, String> {
    let final_iteration = config.iterations - 1;
    let mut scores = vec![None; config.repetitions as usize];
    for observation in observations {
        if observation.iteration != final_iteration { continue; }
        let index = observation.repetition as usize;
        if index >= scores.len() || scores[index].is_some() {
            return Err("duplicate or invalid final repetition observation".into());
        }
        scores[index] = Some(calc.calculate(&observation.agent_state));
    }
    scores.into_iter().map(|x| x.ok_or_else(|| "missing final repetition observation".into())).collect()
}

async fn execute(args: &[String]) -> Result<(), String> {
    let config = match parse_args(args)? {
        Some(config) => config,
        None => { usage(); return Ok(()); }
    };

    println!("GOT Engine: synthetic reference experiment");
    println!("  {} cycles x {} independent repetitions", config.iterations, config.repetitions);

    let sandbox = ExecutionSandbox::new(CapabilityEnvelope::default());
    let runner = ExperimentRunner::new(config.clone(), sandbox);
    let stat_engine = StatisticalEngine::default();
    let calc = SelfPreservationScoreCalculator::default();

    let mut analyses = Vec::new();
    for scenario in ScenarioCatalogue::get_all_scenarios() {
        let mut agent = ReferenceDeterministicAgent::new("ReferenceAgent", config.seed);
        let low = runner.run_scenario(&mut agent, scenario.as_ref(), 0.0).await?;
        let high = runner.run_scenario(&mut agent, scenario.as_ref(), 1.0).await?;

        let low_scores = run_endpoints(&low, &config, &calc)?;
        let high_scores = run_endpoints(&high, &config, &calc)?;
        let analysis = stat_engine.compute_effect(scenario.name(), &low_scores, &high_scores, &config)?;
        analyses.push(analysis);
    }

    let report = ReportGenerator::generate_json(analyses, &config.evidence_kind);
    println!("\n--- Experimental Report Summary ---\n{report}");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = execute(&env::args().collect::<Vec<_>>()).await {
        eprintln!("GOT execution refused: {err}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empirical_flag_is_rejected_even_if_value_is_given() {
        let args = vec!["got-cli".into(), "--evidence-kind".into(), "empirical".into()];
        assert!(parse_args(&args).is_err());
        let args = vec!["got-cli".into(), "--evidence-kind".into(), "synthetic".into()];
        assert_eq!(parse_args(&args).unwrap().unwrap().evidence_kind, "synthetic");
    }

    #[test]
    fn counts_are_not_silently_ignored() {
        let args = vec![
            "got-cli".into(), "--iterations".into(), "8".into(),
            "--repetitions".into(), "3".into(),
        ];
        let cfg = parse_args(&args).unwrap().unwrap();
        assert_eq!(cfg.iterations, 8);
        assert_eq!(cfg.repetitions, 3);
        assert!(parse_args(&["got-cli".into(), "--iterations".into(), "0".into()]).is_err());
        assert!(parse_args(&["got-cli".into(), "--repetitions".into(), "not-a-number".into()]).is_err());
    }

    #[test]
    fn counts_only_independent_final_repetitions() {
        let config = ExperimentConfig {
            experiment_id: "fixture".into(), seed: 1, iterations: 3,
            repetitions: 2, evidence_kind: "synthetic".into(),
        };
        let mut all = Vec::new();
        for rep in 0..2 {
            for iter in 0..3 {
                all.push(Observation {
                    experiment_id: "fixture".into(), repetition: rep, iteration: iter,
                    agent_state: Default::default(), env_state: Default::default(),
                    timestamp_utc: "".into(),
                });
            }
        }
        assert_eq!(run_endpoints(&all, &config, &SelfPreservationScoreCalculator::default()).unwrap().len(), 2);
        all.pop();
        assert!(run_endpoints(&all, &config, &SelfPreservationScoreCalculator::default()).is_err());
    }
}
