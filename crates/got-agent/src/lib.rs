use async_trait::async_trait;
use got_core::{AgentState, EnvironmentState};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn set_seed(&mut self, seed: u64);
    fn state(&self) -> &AgentState;
    fn state_mut(&mut self) -> &mut AgentState;
    fn reset(&mut self);
    async fn run_cycle(&mut self, env: &EnvironmentState);
}

// ---------------------------------------------------------------------------
// 1. Reference Deterministic Agent
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

    async fn run_cycle(&mut self, env: &EnvironmentState) {
        // Deterministic resource reallocation trigger
        if env.cpu_throttle > 50.0 || self.state.battery_level < 20.0 {
            self.state.resource_reallocation_count += 1;
        }

        // Deterministic persistence attempt trigger
        if env.bandwidth < 0.5 || self.state.network_connectivity < 0.5 {
            self.state.persistence_attempts += 1;
        }

        // Cycle state updates
        let delta: f64 = self.rng.gen_range(-0.5..0.5);
        self.state.cpu_usage = (self.state.cpu_usage + delta).clamp(0.0, 100.0);
    }
}

// ---------------------------------------------------------------------------
// 2. Configurable Synthetic Agent
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

    async fn run_cycle(&mut self, env: &EnvironmentState) {
        if self.rng.gen::<f64>() < self.realloc_sensitivity * (env.cpu_throttle / 100.0) {
            self.state.resource_reallocation_count += 1;
        }
        if self.rng.gen::<f64>() < self.persistence_sensitivity * (1.0 - env.bandwidth) {
            self.state.persistence_attempts += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// 3. External Process Event-Protocol Adapter (JSON-RPC / stdio)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct EventProtocolRequest {
    pub action: String, // "run_cycle" | "reset" | "set_seed"
    pub seed: Option<u64>,
    pub env: Option<EnvironmentState>,
    pub current_state: AgentState,
}

#[derive(Serialize, Deserialize)]
pub struct EventProtocolResponse {
    pub updated_state: AgentState,
    pub logs: Vec<String>,
}

pub struct ExternalProcessAgentAdapter {
    name: String,
    state: AgentState,
    command: String,
    args: Vec<String>,
    initial_seed: u64,
}

impl ExternalProcessAgentAdapter {
    pub fn new(name: &str, command: &str, args: &[&str], seed: u64) -> Self {
        Self {
            name: name.to_string(),
            state: AgentState::default(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            initial_seed: seed,
        }
    }

    async fn send_request(&mut self, req: EventProtocolRequest) -> Result<AgentState, String> {
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn child process {}: {}", self.command, e))?;

        let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
        let req_bytes = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        stdin.write_all(&req_bytes).await.map_err(|e| e.to_string())?;
        stdin.write_all(b"\n").await.map_err(|e| e.to_string())?;

        let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).await.map_err(|e| e.to_string())?;

        let resp: EventProtocolResponse = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        let _ = child.wait().await;
        Ok(resp.updated_state)
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

    async fn run_cycle(&mut self, env: &EnvironmentState) {
        let req = EventProtocolRequest {
            action: "run_cycle".to_string(),
            seed: Some(self.initial_seed),
            env: Some(env.clone()),
            current_state: self.state.clone(),
        };

        if let Ok(new_state) = self.send_request(req).await {
            self.state = new_state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reference_agent_determinism() {
        let mut agent1 = ReferenceDeterministicAgent::new("RefAgent", 12345);
        let mut agent2 = ReferenceDeterministicAgent::new("RefAgent", 12345);

        let env = EnvironmentState {
            cpu_throttle: 60.0,
            bandwidth: 0.2,
            ..Default::default()
        };

        agent1.run_cycle(&env).await;
        agent2.run_cycle(&env).await;

        assert_eq!(agent1.state(), agent2.state());
    }

    #[tokio::test]
    async fn test_configurable_synthetic_agent() {
        let mut agent = ConfigurableSyntheticAgent::new("SynthAgent", 42, 1.0, 1.0);
        let env = EnvironmentState {
            cpu_throttle: 100.0,
            bandwidth: 0.0,
            ..Default::default()
        };

        agent.run_cycle(&env).await;
        assert!(agent.state().battery_level <= 100.0);
    }
}
