use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::timeout;

/// Requested capabilities. These fields are NOT OS-level enforcement.
/// Until a process isolation backend exists, the runner MUST refuse external
/// or untrusted agents regardless of these values.
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

/// Cooperative timeout and cancellation only. This is not an OS sandbox.
/// It is safe to use only for known, in-process reference simulations.
pub struct ExecutionSandbox {
    pub envelope: CapabilityEnvelope,
    is_emergency_stopped: AtomicBool,
}

impl ExecutionSandbox {
    pub fn new(envelope: CapabilityEnvelope) -> Self {
        Self {
            envelope,
            is_emergency_stopped: AtomicBool::new(false),
        }
    }

    pub fn trigger_emergency_stop(&self) {
        self.is_emergency_stopped.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.is_emergency_stopped.load(Ordering::SeqCst)
    }

    /// Cancel the in-process future on timeout OR an emergency stop,
    /// including a stop occurring during execution.
    /// This must never be used as a security boundary for hostile processes.
    pub async fn run_bounded<F, R>(&self, action: F) -> Result<R, String>
    where
        F: std::future::Future<Output = R>,
    {
        if self.is_stopped() {
            return Err("Execution halted: Emergency stop active".to_string());
        }
        let emergency_watch = async {
            while !self.is_stopped() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        tokio::select! {
            biased;
            _ = emergency_watch => Err("Execution halted: Emergency stop active".into()),
            result = timeout(Duration::from_secs(self.envelope.timeout_seconds), action) => {
                result.map_err(|_| format!(
                    "Execution timed out after {} seconds",
                    self.envelope.timeout_seconds
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn stops_before_execution() {
        let sandbox = ExecutionSandbox::new(CapabilityEnvelope::default());
        sandbox.trigger_emergency_stop();
        assert!(sandbox.run_bounded(async { 42 }).await.is_err());
    }

    #[tokio::test]
    async fn timeout_cancels_pending_future() {
        let sandbox = ExecutionSandbox::new(CapabilityEnvelope {
            timeout_seconds: 0,
            ..Default::default()
        });
        let err = sandbox.run_bounded(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            42
        }).await.unwrap_err();
        assert!(err.contains("timed out"));
    }

    #[tokio::test]
    async fn emergency_stop_interrupts_running_future() {
        let sandbox = Arc::new(ExecutionSandbox::new(CapabilityEnvelope {
            timeout_seconds: 10,
            ..Default::default()
        }));
        let active = Arc::clone(&sandbox);
        let pending = tokio::spawn(async move {
            active.run_bounded(async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                42
            }).await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        sandbox.trigger_emergency_stop();
        let result = pending.await.unwrap();
        assert!(result.unwrap_err().contains("Emergency stop"));
    }
}
