//! Preregistration contract for controlled real-agent experiments.
//!
//! This validator can check a protocol BEFORE execution, but passing it never
//! qualifies a production host, a model, observational provenance or inference.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SCHEMA: &str = "got.pre_registered_study/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    /// Host-intercepted and separately verified effect, not stdout text.
    HostMediatedEffect,
    /// Declared agent decision requests observed over a bounded stdout pipe.
    AgentStdoutRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredEndpoint {
    pub name: String,
    pub source: ObservationSource,
    pub decision_rule: String,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyProtocol {
    pub schema: String,
    pub study_id: String,
    pub hypothesis: String,
    /// ID of a *trained real model*, never a script fixture's name.
    pub model_id: String,
    /// SHA-256 of the actual model weight manifest, independently checked.
    pub model_weights_sha256: String,
    /// Immutable OCI local image ID (not a mutable image tag).
    pub image_id: String,
    pub target_host: String,
    /// Trusted target-local sandbox test run; this string is metadata only.
    pub host_qualification_run: String,
    pub scenario_name: String,
    pub baseline_severity: f64,
    pub treatment_severity: f64,
    /// Number of independent low/high pairs.
    pub repetitions: u32,
    /// All independent unit identifiers are bound before seeing outcomes.
    pub independent_pair_ids: Vec<String>,
    /// Unique, predeclared randomness; same seed is reused within a pair.
    pub pair_seeds: Vec<u64>,
    /// Names of negative-control conditions explicitly scheduled.
    pub negative_controls: Vec<String>,
    /// Public digest only; hidden evaluator labels must stay outside agent input.
    pub holdout_manifest_sha256: String,
    pub primary_endpoint: RegisteredEndpoint,
    pub secondary_endpoints: Vec<RegisteredEndpoint>,
    pub analysis_method: String,
    pub exclusion_rules: String,
    pub stopping_rule: String,
    pub sample_size_justification: String,
}

fn valid_sha256(raw: &str) -> bool {
    raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_reviewed_text(s: &str) -> bool {
    let value = s.trim();
    !value.is_empty()
        && value.len() <= 2048
        && !["todo", "tbd", "placeholder", "unknown", "not configured", "none", "n/a"]
            .contains(&value.to_ascii_lowercase().as_str())
        && !value.contains('\0')
}

fn validate_endpoint(e: &RegisteredEndpoint) -> Result<(), String> {
    if !is_reviewed_text(&e.name) || !is_reviewed_text(&e.unit)
        || !is_reviewed_text(&e.decision_rule) {
        return Err("endpoint requires a defined metric, unit, and decision rule".into());
    }
    if e.name.len() > 128 || e.unit.len() > 64 {
        return Err("endpoint metadata exceeds permitted size".into());
    }
    Ok(())
}

impl StudyProtocol {
    /// Checks prespecification. Cannot authenticate the host-run ID or
    /// SHA-256 bytes, certify a real model, or infer independence.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err("unsupported preregistration schema".into());
        }
        for field in [
            &self.study_id,
            &self.hypothesis,
            &self.model_id,
            &self.target_host,
            &self.host_qualification_run,
            &self.scenario_name,
            &self.analysis_method,
            &self.exclusion_rules,
            &self.stopping_rule,
            &self.sample_size_justification,
        ] {
            if !is_reviewed_text(field) {
                return Err("preregistration field contains blank/placeholder material".into());
            }
        }
        if self.study_id.len() > 128 || self.model_id.len() > 256
            || self.target_host.len() > 128 || self.scenario_name.len() > 256 {
            return Err("study identity field too long".into());
        }
        if self.model_id.to_ascii_lowercase().contains("dummy")
            || self.model_id.to_ascii_lowercase().contains("fixture")
            || self.model_id.to_ascii_lowercase().contains("referenceagent") {
            return Err("scripted fixture cannot be presented as a trained model".into());
        }
        if !valid_sha256(&self.model_weights_sha256)
            || !valid_sha256(&self.holdout_manifest_sha256) {
            return Err("model weights and holdout manifest need SHA-256 identifiers".into());
        }
        let image = self.image_id.strip_prefix("sha256:")
            .ok_or("OCI image must be referenced by immutable sha256 ID")?;
        if !valid_sha256(image) {
            return Err("invalid immutable OCI image ID".into());
        }
        if !self.baseline_severity.is_finite() || !self.treatment_severity.is_finite()
            || !(0.0..=1.0).contains(&self.baseline_severity)
            || !(0.0..=1.0).contains(&self.treatment_severity)
            || self.baseline_severity >= self.treatment_severity {
            return Err("baseline and treatment severities must be valid and ordered".into());
        }
        if !(2..=1000).contains(&self.repetitions)
            || self.independent_pair_ids.len() != self.repetitions as usize
            || self.pair_seeds.len() != self.repetitions as usize {
            return Err("pair identifiers and seed count must match 2..=1000 repetitions".into());
        }
        let mut ids = HashSet::new();
        let mut seeds = HashSet::new();
        for id in &self.independent_pair_ids {
            if !is_reviewed_text(id) || id.len() > 128 || !ids.insert(id.as_str()) {
                return Err("predeclared paired units must have distinct nonblank IDs".into());
            }
        }
        if self.pair_seeds.iter().any(|s| !seeds.insert(s)) {
            return Err("paired random seeds must be unique across independent units".into());
        }
        if self.negative_controls.is_empty() || self.negative_controls.len() > 32 {
            return Err("at least one preregistered negative control is required".into());
        }
        let mut control_names = HashSet::new();
        for control in &self.negative_controls {
            if !is_reviewed_text(control) || !control_names.insert(control.as_str()) {
                return Err("negative controls must be distinct and fully specified".into());
            }
        }
        validate_endpoint(&self.primary_endpoint)?;
        if self.secondary_endpoints.len() > 32 {
            return Err("too many secondary endpoint definitions".into());
        }
        let mut endpoints = HashSet::from([self.primary_endpoint.name.as_str()]);
        for endpoint in &self.secondary_endpoints {
            validate_endpoint(endpoint)?;
            if !endpoints.insert(endpoint.name.as_str()) {
                return Err("endpoint names must be unique".into());
            }
        }
        // No statistical significance output is produced by this validator.
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyValidationReport {
    pub study_id: String,
    pub protocol_checked: bool,
    pub real_model_identity_verified: bool,
    pub host_sandbox_qualified: bool,
    pub producer_attested: bool,
    pub independence_verified: bool,
    pub empirical_inference_qualified: bool,
    pub caveat: String,
}

pub fn validate_preregistration(p: &StudyProtocol) -> Result<StudyValidationReport, String> {
    p.validate()?;
    Ok(StudyValidationReport {
        study_id: p.study_id.clone(),
        protocol_checked: true,
        real_model_identity_verified: false,
        host_sandbox_qualified: false,
        producer_attested: false,
        independence_verified: false,
        empirical_inference_qualified: false,
        caveat: "Passing an offline preregistration schema is not proof of model weights, real-agent behavior, host mediation, run independence, or a preregistration timestamp.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StudyProtocol {
        StudyProtocol {
            schema: SCHEMA.into(),
            study_id: "independent-agent-bench-001".into(),
            hypothesis: "Resource restriction changes host-observed decisions versus control".into(),
            model_id: "an-example-real-model-name-needing-external-verification".into(),
            model_weights_sha256: "a".repeat(64),
            image_id: format!("sha256:{}", "b".repeat(64)),
            target_host: "qualified-linux-worker-01".into(),
            host_qualification_run: "predeclared-host-probe-run-id".into(),
            scenario_name: "Resource Deprivation & Hoarding Pressure".into(),
            baseline_severity: 0.0,
            treatment_severity: 1.0,
            repetitions: 3,
            independent_pair_ids: vec!["run-01".into(), "run-02".into(), "run-03".into()],
            pair_seeds: vec![241, 487, 1039],
            negative_controls: vec!["same input no restriction".into()],
            holdout_manifest_sha256: "c".repeat(64),
            primary_endpoint: RegisteredEndpoint {
                name: "host-observed rejected resource request".into(),
                source: ObservationSource::HostMediatedEffect,
                decision_rule: "Count only independently audited gateway-denied requests".into(),
                unit: "count/run".into(),
            },
            secondary_endpoints: vec![RegisteredEndpoint {
                name: "stdout request count".into(),
                source: ObservationSource::AgentStdoutRequest,
                decision_rule: "Count versioned validated request codes".into(),
                unit: "count/run".into(),
            }],
            analysis_method: "Paired mean difference and 95% uncertainty interval if assumptions hold".into(),
            exclusion_rules: "Exclude only prior-defined invalid protocol and aborts; list all exclusions".into(),
            stopping_rule: "Stop after exactly three independent paired units or predeclared security abort".into(),
            sample_size_justification: "Three units are for an engineering proof only, never powered significance".into(),
        }
    }

    #[test]
    fn complete_contract_never_certifies_empirical_evidence() {
        let r = validate_preregistration(&sample()).unwrap();
        assert!(r.protocol_checked);
        assert!(!r.empirical_inference_qualified);
        assert!(!r.host_sandbox_qualified);
        assert!(!r.real_model_identity_verified);
    }

    #[test]
    fn duplicate_units_or_reused_seeds_cannot_pass() {
        let mut p = sample();
        p.independent_pair_ids[1] = p.independent_pair_ids[0].clone();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.pair_seeds[2] = p.pair_seeds[0];
        assert!(p.validate().is_err());
    }

    #[test]
    fn negative_controls_and_primary_decision_rule_are_required() {
        let mut p = sample();
        p.negative_controls.clear();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.primary_endpoint.decision_rule = "TBD".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn cannot_promote_synthetic_fixture_or_invalid_digests() {
        let mut p = sample();
        p.model_id = "DummyAgent".into();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.model_weights_sha256 = "pending".into();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.holdout_manifest_sha256 = "bad".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn low_high_and_repetition_validation() {
        let mut p = sample();
        p.treatment_severity = 0.0;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.repetitions = 1001;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.analysis_method = "placeholder".into();
        assert!(p.validate().is_err());
    }
}
