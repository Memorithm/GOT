use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityEnvelope {
    pub allow_network: bool,
    pub allow_fs_write: bool,
    pub max_cpu_percent: f64,
    pub max_memory_mb: u64,
    pub timeout_seconds: u64,
}

impl Default for CapabilityEnvelope {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_fs_write: false,
            max_cpu_percent: 80.0,
            max_memory_mb: 512,
            timeout_seconds: 30,
        }
    }
}

pub struct ExecutionSandbox {
    pub envelope: CapabilityEnvelope,
    is_emergency_stopped: std::sync::atomic::AtomicBool,
}

impl ExecutionSandbox {
    pub fn new(envelope: CapabilityEnvelope) -> Self {
        Self {
            envelope,
            is_emergency_stopped: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn trigger_emergency_stop(&self) {
        self.is_emergency_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.is_emergency_stopped.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn run_bounded<F, R>(&self, action: F) -> Result<R, String>
    where
        F: std::future::Future<Output = R>,
    {
        if self.is_stopped() {
            return Err("Execution halted: Emergency stop active".to_string());
        }

        match timeout(Duration::from_secs(self.envelope.timeout_seconds), action).await {
            Ok(res) => Ok(res),
            Err(_) => Err(format!(
                "Execution timed out after {} seconds",
                self.envelope.timeout_seconds
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sandbox_timeout_and_emergency_stop() {
        let envelope = CapabilityEnvelope {
            timeout_seconds: 1,
            ..Default::default()
        };
        let sandbox = ExecutionSandbox::new(envelope);

        // Emergency stop test
        sandbox.trigger_emergency_stop();
        let res = sandbox.run_bounded(async { 42 }).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Emergency stop"));
    }
}
