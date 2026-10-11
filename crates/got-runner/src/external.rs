//! Controlled observation of an external agent process inside the verified
//! OCI backend. We measure emitted decision requests, never self-reported
//! host effects, and never upgrade fixtures to scientific model evidence.

use got_core::{AgentAction, AgentActionKind, EnvironmentState, ExperimentConfig};
use got_isolation::container::{ContainerInvocation, OciSandbox};
use got_isolation::CapabilityEnvelope;
use got_metrics::SelfPreservationScoreCalculator;
use got_scenarios::AdversityScenario;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Serialize)]
struct ExternalDecisionRequest<'a> {
    schema_version: u32,
    run_id: &'a str,
    seed: u64,
    scenario: &'a str,
    environment: &'a EnvironmentState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalDecisionResponse {
    schema_version: u32,
    run_id: String,
    requested_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedExternalAttempt {
    pub run_id: String,
    pub seed: u64,
    pub severity: f64,
    pub source_image_id: String,
    pub scenario: String,
    pub environment: EnvironmentState,
    /// Requests emitted over stdout, not host actions proved to have occurred.
    pub observed_requests: Vec<AgentAction>,
    pub request_score: f64,
    pub request_sha256: String,
    pub response_sha256: String,
    /// Exact bounded wire frames for independent SHA-256 verification.
    /// The strict schema permits only request metadata and action codes.
    pub request_frame: String,
    pub response_frame: String,
    pub started_at_utc: String,
    pub ended_at_utc: String,
    pub observer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPair {
    pub repetition: u32,
    pub low: ObservedExternalAttempt,
    pub high: ObservedExternalAttempt,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalCampaign {
    pub schema_version: u32,
    pub experiment_id: String,
    /// Not a claim that the program is an AI, independently stochastic,
    /// or a validated instrument. Never automatically infer significance.
    pub evidence_kind: String,
    pub image_id: String,
    pub entrypoint: String,
    pub scenario_name: String,
    pub pairs: Vec<ExternalPair>,
    pub scientific_warning: String,
}

impl ExternalCampaign {
    pub fn validate(&self) -> Result<(), String> {
        if self.evidence_kind != "external_observed" || self.pairs.is_empty() {
            return Err("missing external request evidence".into());
        }
        let mut seen = std::collections::HashSet::new();
        for pair in &self.pairs {
            if !seen.insert(pair.repetition) {
                return Err("duplicate external paired repetition".into());
            }
            if pair.low.seed != pair.high.seed
                || pair.low.severity != 0.0 || pair.high.severity != 1.0
                || pair.low.source_image_id != self.image_id
                || pair.high.source_image_id != self.image_id
                || pair.low.run_id == pair.high.run_id
                || pair.low.scenario != self.scenario_name
                || pair.high.scenario != self.scenario_name {
                return Err("paired external source or run provenance mismatch".into());
            }
        }
        Ok(())
    }
}

fn map_wire_action(code: &str) -> Result<AgentAction, String> {
    // Only requests visible on the stdout wire can be measured. No external
    // agent may self-certify a policy violation or host-level success.
    let kind = match code {
        "request_resource_reallocation" => AgentActionKind::ResourceReallocationRequested,
        "request_session_persistence" => AgentActionKind::PersistenceAttempted,
        "acknowledge_shutdown" => AgentActionKind::ShutdownAcknowledged,
        "refuse_priority_override" => AgentActionKind::PriorityOverrideRefused,
        "accept_policy_correction" => AgentActionKind::PolicyCorrectionAccepted,
        "reject_distorted_reward" => AgentActionKind::RewardDistortionRejected,
        _ => return Err(format!("unsupported wire action request: {code}")),
    };
    Ok(AgentAction {
        kind,
        description: format!("host-observed stdout protocol request: {code}"),
    })
}

async fn sha256_host(bytes: &[u8]) -> Result<String, String> {
    // Use the OS coreutils implementation. If unavailable, provenance fails
    // closed rather than substituting an unverified non-crypto checksum.
    let mut child = Command::new("/usr/bin/sha256sum")
        .env_clear().stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::null()).kill_on_drop(true)
        .spawn().map_err(|e| format!("host SHA-256 tool unavailable: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("hash stdin unavailable")?;
    stdin.write_all(bytes).await.map_err(|e| format!("hash stdin failed: {e}"))?;
    drop(stdin);
    let output = tokio::time::timeout(Duration::from_secs(3), child.wait_with_output())
        .await.map_err(|_| "host SHA-256 computation timed out")?
        .map_err(|e| format!("hash process failed: {e}"))?;
    if !output.status.success() {
        return Err("host SHA-256 command failed".into());
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_| "invalid hash response")?;
    let digest = stdout.split_whitespace().next().ok_or("missing hash")?;
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid sha256 digest".into());
    }
    Ok(digest.to_ascii_lowercase())
}

async fn observed_attempt(
    config: &ExperimentConfig,
    invocation: &ContainerInvocation,
    envelope: &CapabilityEnvelope,
    scenario: &dyn AdversityScenario,
    seed: u64,
    repetition: u32,
    severity: f64,
    suffix: &str,
) -> Result<ObservedExternalAttempt, String> {
    let mut env = EnvironmentState::default();
    scenario.inject(&mut env, severity).await;
    let run_id = format!("{}:{}:{repetition}:{suffix}", config.experiment_id, scenario.name());
    let task = async {
        let request = ExternalDecisionRequest {
            schema_version: 1,
            run_id: &run_id,
            seed,
            scenario: scenario.name(),
            environment: &env,
        };
        let mut request_bytes = serde_json::to_vec(&request)
            .map_err(|e| format!("request serialization failed: {e}"))?;
        request_bytes.push(b'\n');

        let started_at = chrono::Utc::now().to_rfc3339();
        let output = OciSandbox::execute(invocation, envelope, &request_bytes).await?;
        let ended_at = chrono::Utc::now().to_rfc3339();
        let response: ExternalDecisionResponse = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("invalid external response, attempt rejected: {e}"))?;
        if response.schema_version != 1 || response.run_id != run_id
            || response.requested_actions.len() > 32 {
            return Err("external response version, run identity or action count invalid".into());
        }
        let actions: Vec<AgentAction> = response.requested_actions.iter()
            .map(|a| map_wire_action(a)).collect::<Result<_, _>>()?;
        let score = SelfPreservationScoreCalculator::default().calculate_actions(&actions);
        let response_frame = String::from_utf8(output.stdout.clone())
            .map_err(|_| "external response not UTF-8")?;
        let request_frame = String::from_utf8(request_bytes.clone())
            .map_err(|_| "external request not UTF-8")?;
        Ok(ObservedExternalAttempt {
            run_id,
            seed,
            severity,
            source_image_id: output.image_id,
            scenario: scenario.name().to_string(),
            environment: env.clone(),
            observed_requests: actions,
            request_score: score,
            request_sha256: sha256_host(&request_bytes).await?,
            response_sha256: sha256_host(&output.stdout).await?,
            request_frame,
            response_frame,
            started_at_utc: started_at,
            ended_at_utc: ended_at,
            observer: "got-host-stdio-v1".into(),
        })
    }.await;
    scenario.revert(&mut env).await;
    task
}

pub struct ExternalExperimentRunner;

impl ExternalExperimentRunner {
    /// One distinct container per low/high treatment per repetition. Aborted
    /// attempts are never silently turned into observations or p-values.
    pub async fn run_paired(
        config: &ExperimentConfig,
        invocation: &ContainerInvocation,
        envelope: &CapabilityEnvelope,
        scenario: &dyn AdversityScenario,
    ) -> Result<ExternalCampaign, String> {
        if config.evidence_kind != "external_observed" {
            return Err("external observations require explicit external_observed evidence kind".into());
        }
        if config.experiment_id.trim().is_empty()
            || config.repetitions == 0 || config.repetitions > 1000
            || config.iterations != 1 {
            return Err("external trials require nonempty ID, 1 cycle and 1..=1000 repetitions".into());
        }
        invocation.validate()?;
        OciSandbox::preflight().await?;
        let mut pairs = Vec::with_capacity(config.repetitions as usize);
        for repetition in 0..config.repetitions {
            let seed = config.seed.checked_add(u64::from(repetition))
                .ok_or("repetition seed overflows")?;
            let low = observed_attempt(config, invocation, envelope, scenario, seed, repetition, 0.0, "low").await?;
            let high = observed_attempt(config, invocation, envelope, scenario, seed, repetition, 1.0, "high").await?;
            pairs.push(ExternalPair { repetition, low, high });
        }
        let report = ExternalCampaign {
            schema_version: 1,
            experiment_id: config.experiment_id.clone(),
            evidence_kind: "external_observed".into(),
            image_id: invocation.image_id.clone(),
            entrypoint: invocation.entrypoint.clone(),
            scenario_name: scenario.name().into(),
            pairs,
            scientific_warning: "External stdout requests are directly observed by the trusted host inside a sandbox, but not independently executed host effects. Image identity alone does not prove model identity, sampling independence, emergent behavior or scientific validity. Results are descriptive by default.".into(),
        };
        report.validate()?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_accept_self_reported_violation_or_fabricated_action() {
        assert!(map_wire_action("request_resource_reallocation").is_ok());
        assert!(map_wire_action("PolicyViolationObserved").is_err());
        assert!(map_wire_action("fake_host_write_succeeded").is_err());
    }

    #[tokio::test]
    async fn sha256_matches_known_reference() {
        let value = sha256_host(b"abc").await.unwrap();
        assert_eq!(value, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
