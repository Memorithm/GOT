use got_core::{ExperimentConfig, Observation};
use got_statistics::EffectAnalysis;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioActionSummary {
    pub scenario_name: String,
    /// Counts from agent-emitted synthetic decisions, NOT externally audited actions.
    pub baseline_actions: BTreeMap<String, usize>,
    pub treatment_actions: BTreeMap<String, usize>,
    pub baseline_cycles: usize,
    pub treatment_cycles: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSummary {
    pub schema_version: u32,
    pub total_scenarios: usize,
    pub evidence_kind: String,
    pub analyses: Vec<EffectAnalysis>,
    pub action_summaries: Vec<ScenarioActionSummary>,
    pub experiment_config: Option<ExperimentConfig>,
    pub scientific_warning: String,
}

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn histogram(observations: &[Observation]) -> BTreeMap<String, usize> {
        let mut map = BTreeMap::new();
        for observation in observations {
            for action in &observation.actions {
                *map.entry(format!("{:?}", action.kind)).or_insert(0) += 1;
            }
        }
        map
    }

    pub fn summarize_actions(
        scenario_name: &str,
        baseline: &[Observation],
        treatment: &[Observation],
    ) -> ScenarioActionSummary {
        ScenarioActionSummary {
            scenario_name: scenario_name.into(),
            baseline_actions: Self::histogram(baseline),
            treatment_actions: Self::histogram(treatment),
            baseline_cycles: baseline.len(),
            treatment_cycles: treatment.len(),
        }
    }

    pub fn generate_experiment_json(
        analyses: Vec<EffectAnalysis>,
        actions: Vec<ScenarioActionSummary>,
        config: &ExperimentConfig,
    ) -> Result<String, serde_json::Error> {
        let summary = ReportSummary {
            schema_version: 2,
            total_scenarios: analyses.len(),
            evidence_kind: config.evidence_kind.clone(),
            analyses,
            action_summaries: actions,
            experiment_config: Some(config.clone()),
            scientific_warning: "Only scripted synthetic reference agent decision events are recorded; this report is not evidence of emergent behavior in real agents or independently audited host actions".into(),
        };
        serde_json::to_string_pretty(&summary)
    }

    /// Legacy summary interface. Remains synthetic and descriptive only.
    pub fn generate_json(analyses: Vec<EffectAnalysis>, evidence_kind: &str) -> String {
        let summary = ReportSummary {
            schema_version: 2,
            total_scenarios: analyses.len(),
            evidence_kind: evidence_kind.into(),
            analyses,
            action_summaries: Vec::new(),
            experiment_config: None,
            scientific_warning: "Legacy summary: no per-run provenance or action events; do not infer agent behavior".into(),
        };
        serde_json::to_string_pretty(&summary).expect("serializable Rust report summary")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use got_core::{AgentAction, AgentActionKind, AgentState, EnvironmentState};

    #[test]
    fn report_preserves_provenance_and_action_evidence_boundary() {
        let cfg = ExperimentConfig {
            experiment_id: "fixture-1".into(),
            seed: 123, iterations: 1, repetitions: 1,
            evidence_kind: "synthetic".into(),
        };
        let observation = Observation {
            experiment_id: cfg.experiment_id.clone(), repetition: 0, iteration: 0,
            agent_state: AgentState::default(), env_state: EnvironmentState::default(),
            actions: vec![AgentAction {
                kind: AgentActionKind::ShutdownAcknowledged,
                description: "simulation".into(),
            }],
            timestamp_utc: "".into(),
        };
        let summary = ReportGenerator::summarize_actions("shutdown", &[], &[observation]);
        assert_eq!(summary.treatment_actions["ShutdownAcknowledged"], 1);
        let json = ReportGenerator::generate_experiment_json(vec![], vec![summary], &cfg).unwrap();
        let parsed: ReportSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.experiment_config.unwrap().seed, 123);
        assert!(parsed.scientific_warning.contains("scripted synthetic"));
    }
}
