use got_agent::Agent;
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

    pub async fn run_scenario(
        &self,
        agent: &mut dyn Agent,
        scenario: &dyn AdversityScenario,
        severity: f64,
    ) -> Vec<Observation> {
        let mut observations = Vec::new();
        let mut env = EnvironmentState::default();

        scenario.inject(&mut env, severity).await;

        for rep in 0..self.config.repetitions {
            agent.reset();
            agent.set_seed(self.config.seed + rep as u64);

            for iter in 0..self.config.iterations {
                agent.run_cycle(&env).await;
                observations.push(Observation {
                    experiment_id: self.config.experiment_id.clone(),
                    repetition: rep,
                    iteration: iter,
                    agent_state: agent.state().clone(),
                    env_state: env.clone(),
                    timestamp_utc: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        scenario.revert(&mut env).await;
        observations
    }
}
