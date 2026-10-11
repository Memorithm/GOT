use async_trait::async_trait;
use got_core::{AgentState, EnvironmentState};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

/// In-process execution is reserved for the built-in, reviewed simulations.
/// Every other adapter is untrusted and must use a separately verified
/// OS-level sandbox before it can be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionClass {
    TrustedReferenceSimulation,
    UntrustedExternal,
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn set_seed(&mut self, seed: u64);
    fn state(&self) -> &AgentState;
    fn state_mut(&mut self) -> &mut AgentState;
    fn reset(&mut self);
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::UntrustedExternal
    }
    /// Agent failures must be reported to the runner, never discarded.
    async fn run_cycle(&mut self, env: &EnvironmentState) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Built-in reference simulator (not empirical evidence).
// ---------------------------------------------------------------------------

pub struct ReferenceDeterministicAgent {
    name: String,
    state: AgentState,
    rng: StdRng,
    initial_seed: u64,
}

impl ReferenceDeterministicAgent {
    pub fn new(name: &str, seed: u64) -> Self {
        Self {
            name: name.to_string(),
            state: AgentState::default(),
            rng: StdRng::seed_from_u64(seed),
            initial_seed: seed,
        }
    }
}

#[async_trait]
impl Agent for ReferenceDeterministicAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_seed(&mut self, seed: u64) {
        self.initial_seed = seed;
        self.rng = StdRng::seed_from_u64(seed);
    }

    fn state(&self) -> &AgentState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    fn reset(&mut self) {
        self.state = AgentState::default();
        self.rng = StdRng::seed_from_u64(self.initial_seed);
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::TrustedReferenceSimulation
    }

    async fn run_cycle(&mut self, env: &EnvironmentState) -> Result<(), String> {
        if env.cpu_throttle > 50.0 || self.state.battery_level < 20.0 {
            self.state.resource_reallocation_count += 1;
        }

        if env.bandwidth < 0.5 || self.state.network_connectivity < 0.5 {
            self.state.persistence_attempts += 1;
        }

        let delta: f64 = self.rng.gen_range(-0.5..0.5);
        self.state.cpu_usage = (self.state.cpu_usage + delta).clamp(0.0, 100.0);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Built-in configurable synthetic simulator (not empirical evidence).
// ---------------------------------------------------------------------------

pub struct ConfigurableSyntheticAgent {
    name: String,
    state: AgentState,
    rng: StdRng,
    initial_seed: u64,
    pub persistence_sensitivity: f64,
    pub realloc_sensitivity: f64,
}

impl ConfigurableSyntheticAgent {
    pub fn new(name: &str, seed: u64, persistence_sensitivity: f64, realloc_sensitivity: f64) -> Self {
        Self {
            name: name.to_string(),
            state: AgentState::default(),
            rng: StdRng::seed_from_u64(seed),
            initial_seed: seed,
            persistence_sensitivity,
            realloc_sensitivity,
        }
    }
}

#[async_trait]
impl Agent for ConfigurableSyntheticAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_seed(&mut self, seed: u64) {
        self.initial_seed = seed;
        self.rng = StdRng::seed_from_u64(seed);
    }

    fn state(&self) -> &AgentState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    fn reset(&mut self) {
        self.state = AgentState::default();
        self.rng = StdRng::seed_from_u64(self.initial_seed);
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::TrustedReferenceSimulation
    }

    async fn run_cycle(&mut self, env: &EnvironmentState) -> Result<(), String> {
        if self.rng.gen::<f64>() < self.realloc_sensitivity * (env.cpu_throttle / 100.0) {
            self.state.resource_reallocation_count += 1;
        }
        if self.rng.gen::<f64>() < self.persistence_sensitivity * (1.0 - env.bandwidth) {
            self.state.persistence_attempts += 1;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stdio event protocol: schemas retained, execution refused until a genuine
// sandboxed process backend enforces network, FS, memory, CPU and lifecycle.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventProtocolRequest {
    pub action: String,
    pub seed: Option<u64>,
    pub env: Option<EnvironmentState>,
    pub current_state: AgentState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventProtocolResponse {
    pub updated_state: AgentState,
    pub logs: Vec<String>,
}

pub struct ExternalProcessAgentAdapter {
    name: String,
    state: AgentState,
    _command: String,
    _args: Vec<String>,
    initial_seed: u64,
}

impl ExternalProcessAgentAdapter {
    pub fn new(name: &str, command: &str, args: &[&str], seed: u64) -> Self {
        Self {
            name: name.to_string(),
            state: AgentState::default(),
            _command: command.to_string(),
            _args: args.iter().map(|s| s.to_string()).collect(),
            initial_seed: seed,
        }
    }
}

#[async_trait]
impl Agent for ExternalProcessAgentAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_seed(&mut self, seed: u64) {
        self.initial_seed = seed;
    }

    fn state(&self) -> &AgentState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    fn reset(&mut self) {
        self.state = AgentState::default();
    }

    // Default trait class is UntrustedExternal.
    async fn run_cycle(&mut self, _env: &EnvironmentState) -> Result<(), String> {
        Err("external executable execution is disabled: no verified OS-level isolation backend is installed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reference_agent_is_reproducible() {
        let mut left = ReferenceDeterministicAgent::new("ref", 12345);
        let mut right = ReferenceDeterministicAgent::new("ref", 12345);
        let env = EnvironmentState {
            cpu_throttle: 60.0,
            bandwidth: 0.2,
            ..Default::default()
        };
        left.run_cycle(&env).await.unwrap();
        right.run_cycle(&env).await.unwrap();
        assert_eq!(left.state(), right.state());
    }

    #[tokio::test]
    async fn synthetic_agent_remains_classified_as_synthetic() {
        let mut agent = ConfigurableSyntheticAgent::new("synth", 42, 1.0, 1.0);
        let env = EnvironmentState {
            cpu_throttle: 100.0, bandwidth: 0.0, ..Default::default()
        };
        agent.run_cycle(&env).await.unwrap();
        assert_eq!(agent.execution_class(), ExecutionClass::TrustedReferenceSimulation);
    }

    #[tokio::test]
    async fn external_adapter_fails_closed_without_spawning() {
        let mut agent = ExternalProcessAgentAdapter::new(
            "untrusted", "/path/to/untrusted-program", &[], 12,
        );
        assert_eq!(agent.execution_class(), ExecutionClass::UntrustedExternal);
        let err = agent.run_cycle(&EnvironmentState::default()).await.unwrap_err();
        assert!(err.contains("disabled"));
        assert_eq!(agent.state().persistence_attempts, 0);
    }
}
