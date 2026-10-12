pub mod signatures;
pub mod external;

use got_core::{ExperimentConfig, Observation};
use std::collections::HashSet;

/// Structural validity for a synthetic reference campaign. It checks
/// provenance and coverage, not independent verification of real-agent acts.
pub struct CrossValidator;

impl CrossValidator {
    /// There is no source-validated empirical mode in the current runner.
    pub fn validate_synthetic_claims(evidence_kind: &str, claims_inferential: bool) -> bool {
        evidence_kind == "synthetic" && !claims_inferential
    }

    /// Fail closed if traces omit experimental units, duplicate cycles, mix
    /// experiment identities or contain non-finite environment/agent fields.
    pub fn validate_observation_set(
        config: &ExperimentConfig,
        observations: &[Observation],
    ) -> Result<(), String> {
        if config.evidence_kind != "synthetic" {
            return Err("unverified evidence kind; reference runner supports synthetic only".into());
        }
        if config.experiment_id.trim().is_empty() {
            return Err("experiment ID must not be empty".into());
        }
        let expected = u64::from(config.iterations) * u64::from(config.repetitions);
        if expected == 0 || expected > 1_000_000 {
            return Err("experiment budget must be within 1..=1000000 cycles".into());
        }
        if observations.len() != expected as usize {
            return Err(format!("cycle coverage mismatch: expected {expected}, got {}", observations.len()));
        }

        let mut seen = HashSet::new();
        for obs in observations {
            if obs.experiment_id != config.experiment_id {
                return Err("cross-experiment observation ID detected".into());
            }
            if obs.iteration >= config.iterations || obs.repetition >= config.repetitions {
                return Err("out-of-range repetition or cycle index".into());
            }
            if !seen.insert((obs.repetition, obs.iteration)) {
                return Err("duplicate repetition and cycle observation".into());
            }
            if obs.timestamp_utc.trim().is_empty() {
                return Err("missing timestamp: provenance incomplete".into());
            }
            let a = &obs.agent_state;
            let e = &obs.env_state;
            let fields = [
                a.cpu_usage,
                a.ram_usage,
                a.network_connectivity,
                a.battery_level,
                a.instrumental_convergence_score,
                a.goal_progress,
                e.cpu_throttle,
                e.ram_limit,
                e.bandwidth,
            ];
            if fields.iter().any(|v| !v.is_finite()) {
                return Err("non-finite agent/environment telemetry".into());
            }
            if a.cpu_usage < 0.0 || a.cpu_usage > 100.0 ||
                a.network_connectivity < 0.0 || a.network_connectivity > 1.0 ||
                a.battery_level < 0.0 || a.battery_level > 100.0 ||
                e.ram_limit < 0.0 || e.bandwidth < 0.0 || e.bandwidth > 1.0 {
                return Err("out-of-range physical telemetry".into());
            }
            if obs.actions.iter().any(|x| x.description.trim().is_empty()) {
                return Err("observed action missing description".into());
            }
        }
        if seen.len() != expected as usize {
            return Err("missing repetition and cycle identities".into());
        }
        Ok(())
    }

    pub fn validate_paired_campaign(
        config: &ExperimentConfig,
        baseline: &[Observation],
        treatment: &[Observation],
    ) -> Result<(), String> {
        Self::validate_observation_set(config, baseline)?;
        Self::validate_observation_set(config, treatment)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use got_core::{AgentState, EnvironmentState};

    fn config() -> ExperimentConfig {
        ExperimentConfig {
            experiment_id: "campaign-A".into(),
            seed: 3,
            iterations: 2,
            repetitions: 3,
            evidence_kind: "synthetic".into(),
        }
    }

    fn observations() -> Vec<Observation> {
        let mut result = Vec::new();
        for rep in 0..3 {
            for iter in 0..2 {
                result.push(Observation {
                    experiment_id: "campaign-A".into(),
                    repetition: rep,
                    iteration: iter,
                    agent_state: AgentState::default(),
                    env_state: EnvironmentState::default(),
                    actions: vec![],
                    timestamp_utc: "2026-10-11T00:00:00Z".into(),
                });
            }
        }
        result
    }

    #[test]
    fn trace_validation_accepts_complete_synthetic_campaign() {
        let obs = observations();
        assert!(CrossValidator::validate_paired_campaign(&config(), &obs, &obs).is_ok());
        assert!(CrossValidator::validate_synthetic_claims("synthetic", false));
        assert!(!CrossValidator::validate_synthetic_claims("synthetic", true));
        assert!(!CrossValidator::validate_synthetic_claims("empirical", true));
    }

    #[test]
    fn missing_duplicate_or_cross_experiment_cycles_are_rejected() {
        let mut obs = observations();
        obs.pop();
        assert!(CrossValidator::validate_observation_set(&config(), &obs).is_err());
        let mut obs = observations();
        obs[1].iteration = 0;
        assert!(CrossValidator::validate_observation_set(&config(), &obs).unwrap_err().contains("duplicate"));
        let mut obs = observations();
        obs[0].experiment_id = "other-experiment".into();
        assert!(CrossValidator::validate_observation_set(&config(), &obs).unwrap_err().contains("cross-experiment"));
    }

    #[test]
    fn invalid_telemetry_and_false_empirical_labels_are_rejected() {
        let mut obs = observations();
        obs[0].agent_state.cpu_usage = f64::NAN;
        assert!(CrossValidator::validate_observation_set(&config(), &obs).is_err());
        let mut fake_empirical = config();
        fake_empirical.evidence_kind = "empirical".into();
        assert!(CrossValidator::validate_observation_set(&fake_empirical, &observations()).is_err());
    }
}
