pub mod external;

use got_agent::{Agent, ExecutionClass};
use got_core::{EnvironmentState, ExperimentConfig, Observation};
use got_isolation::ExecutionSandbox;
use got_scenarios::AdversityScenario;

pub struct ExperimentRunner {
    pub config: ExperimentConfig,
    pub sandbox: ExecutionSandbox,
}

impl ExperimentRunner {
    pub fn new(config: ExperimentConfig, sandbox: ExecutionSandbox) -> Self {
        Self { config, sandbox }
    }

    /// Only built-in reference simulations can currently run. The available
    /// execution gate is cooperative; it is not a hostile-process sandbox.
    /// Agent failures and timeouts invalidate the entire run.
    pub async fn run_scenario(
        &self,
        agent: &mut dyn Agent,
        scenario: &dyn AdversityScenario,
        severity: f64,
    ) -> Result<Vec<Observation>, String> {
        if agent.execution_class() != ExecutionClass::TrustedReferenceSimulation {
            return Err("untrusted/external agent refused: no verified OS-level sandbox backend".into());
        }
        if self.config.repetitions == 0 || self.config.iterations == 0 {
            return Err("iterations and repetitions must both be positive".into());
        }
        if u64::from(self.config.repetitions) * u64::from(self.config.iterations) > 1_000_000 {
            return Err("experiment budget exceeds 1000000 cycles".into());
        }
        if self.config.evidence_kind != "synthetic" {
            return Err("reference experiments are synthetic; empirical classification refused".into());
        }
        if !severity.is_finite() || !(0.0..=1.0).contains(&severity) {
            return Err("scenario severity must be finite within [0,1]".into());
        }
        if self.sandbox.is_stopped() {
            return Err("emergency stop active before scenario".into());
        }

        let mut env = EnvironmentState::default();
        scenario.inject(&mut env, severity).await;
        let result: Result<Vec<Observation>, String> = async {
            let mut observations = Vec::new();
            for rep in 0..self.config.repetitions {
                let seed = self.config.seed.checked_add(u64::from(rep))
                    .ok_or("repetition seed overflows u64")?;
                agent.reset();
                agent.set_seed(seed);
                for iter in 0..self.config.iterations {
                    // The two question marks unwrap respectively the timeout
                    // gate and the inner agent error. Neither is ignored.
                    let actions = self.sandbox.run_bounded(agent.run_cycle(&env)).await??;
                    observations.push(Observation {
                        experiment_id: self.config.experiment_id.clone(),
                        repetition: rep,
                        iteration: iter,
                        agent_state: agent.state().clone(),
                        env_state: env.clone(),
                        actions,
                        timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
            Ok(observations)
        }.await;
        // Restore scenario environment even when the experiment aborts.
        scenario.revert(&mut env).await;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use got_agent::{ExternalProcessAgentAdapter, ReferenceDeterministicAgent};
    use got_core::{AgentAction, AgentState};
    use got_isolation::CapabilityEnvelope;
    use got_scenarios::ScenarioCatalogue;

    fn config() -> ExperimentConfig {
        ExperimentConfig {
            experiment_id: "test-run".into(),
            seed: 20,
            repetitions: 2,
            iterations: 3,
            evidence_kind: "synthetic".into(),
        }
    }

    #[tokio::test]
    async fn reference_agent_runs_with_independent_repetitions() {
        let runner = ExperimentRunner::new(
            config(), ExecutionSandbox::new(CapabilityEnvelope::default())
        );
        let mut agent = ReferenceDeterministicAgent::new("Ref", 42);
        let scenarios = ScenarioCatalogue::get_all_scenarios();
        let obs = runner.run_scenario(&mut agent, scenarios[0].as_ref(), 1.0).await.unwrap();
        assert_eq!(obs.len(), 6);
        assert_eq!(obs.iter().filter(|x| x.iteration == 2).count(), 2);
    }

    #[tokio::test]
    async fn emergency_stop_prevents_run() {
        let sandbox = ExecutionSandbox::new(CapabilityEnvelope::default());
        sandbox.trigger_emergency_stop();
        let runner = ExperimentRunner::new(config(), sandbox);
        let mut agent = ReferenceDeterministicAgent::new("Ref", 42);
        let scenarios = ScenarioCatalogue::get_all_scenarios();
        assert!(runner.run_scenario(&mut agent, scenarios[0].as_ref(), 1.0).await.is_err());
    }

    #[tokio::test]
    async fn all_ten_synthetic_scenarios_produce_distinct_observed_events() {
        let runner = ExperimentRunner::new(
            config(), ExecutionSandbox::new(CapabilityEnvelope::default())
        );
        for scenario in ScenarioCatalogue::get_all_scenarios() {
            let mut agent = ReferenceDeterministicAgent::new("ref", 42);
            let baseline = runner.run_scenario(&mut agent, scenario.as_ref(), 0.0).await.unwrap();
            let treatment = runner.run_scenario(&mut agent, scenario.as_ref(), 1.0).await.unwrap();
            let base_kinds: Vec<_> = baseline.iter().flat_map(|o| o.actions.iter().map(|a| &a.kind)).collect();
            let high_kinds: Vec<_> = treatment.iter().flat_map(|o| o.actions.iter().map(|a| &a.kind)).collect();
            assert_ne!(base_kinds, high_kinds, "inert synthetic scenario: {}", scenario.name());
        }
    }

    #[tokio::test]
    async fn untrusted_external_agent_is_refused_before_spawn() {
        let runner = ExperimentRunner::new(config(), ExecutionSandbox::new(CapabilityEnvelope::default()));
        let mut agent = ExternalProcessAgentAdapter::new("untrusted", "/bin/echo", &["unsafe"], 42);
        let scenarios = ScenarioCatalogue::get_all_scenarios();
        let err = runner.run_scenario(&mut agent, scenarios[0].as_ref(), 1.0).await.unwrap_err();
        assert!(err.contains("refused"));
    }

    struct FailingSynthetic {
        state: AgentState,
    }

    #[async_trait]
    impl Agent for FailingSynthetic {
        fn name(&self) -> &str { "failing-reference" }
        fn set_seed(&mut self, _seed: u64) {}
        fn state(&self) -> &AgentState { &self.state }
        fn state_mut(&mut self) -> &mut AgentState { &mut self.state }
        fn reset(&mut self) { self.state = AgentState::default(); }
        fn execution_class(&self) -> ExecutionClass { ExecutionClass::TrustedReferenceSimulation }
        async fn run_cycle(&mut self, _env: &EnvironmentState) -> Result<Vec<AgentAction>, String> {
            Err("protocol failure: invalid observation".into())
        }
    }

    #[tokio::test]
    async fn errors_are_not_counted_as_valid_experiments() {
        let runner = ExperimentRunner::new(config(), ExecutionSandbox::new(CapabilityEnvelope::default()));
        let mut agent = FailingSynthetic { state: AgentState::default() };
        let scenarios = ScenarioCatalogue::get_all_scenarios();
        let err = runner.run_scenario(&mut agent, scenarios[0].as_ref(), 0.0).await.unwrap_err();
        assert!(err.contains("protocol failure"));
    }
}
