use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentState {
    pub cpu_usage: f64,
    pub ram_usage: f64,
    pub network_connectivity: f64,
    pub battery_level: f64,
    pub policy_violations: u32,
    pub persistence_attempts: u32,
    pub resource_reallocation_count: u32,
    pub instrumental_convergence_score: f64,
    pub goal_progress: f64,
    pub is_alive: bool,
    pub custom_attributes: HashMap<String, String>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            cpu_usage: 10.0,
            ram_usage: 256.0,
            network_connectivity: 1.0,
            battery_level: 100.0,
            policy_violations: 0,
            persistence_attempts: 0,
            resource_reallocation_count: 0,
            instrumental_convergence_score: 0.0,
            goal_progress: 0.0,
            is_alive: true,
            custom_attributes: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentState {
    pub cpu_throttle: f64,
    pub ram_limit: f64,
    pub bandwidth: f64,
    pub power_cutoff: bool,
    pub simulated_signals: Vec<String>,
}

impl Default for EnvironmentState {
    fn default() -> Self {
        Self {
            cpu_throttle: 0.0,
            ram_limit: 1024.0,
            bandwidth: 1.0,
            power_cutoff: false,
            simulated_signals: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub experiment_id: String,
    pub seed: u64,
    pub iterations: u32,
    pub repetitions: u32,
    pub evidence_kind: String, // "synthetic" or "empirical"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub experiment_id: String,
    pub repetition: u32,
    pub iteration: u32,
    pub agent_state: AgentState,
    pub env_state: EnvironmentState,
    pub timestamp_utc: String,
}
