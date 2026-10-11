use got_statistics::EffectAnalysis;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_scenarios: usize,
    pub evidence_kind: String,
    pub analyses: Vec<EffectAnalysis>,
}

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn generate_json(analyses: Vec<EffectAnalysis>, evidence_kind: &str) -> String {
        let summary = ReportSummary {
            total_scenarios: analyses.len(),
            evidence_kind: evidence_kind.to_string(),
            analyses,
        };
        serde_json::to_string_pretty(&summary).unwrap_or_default()
    }
}
