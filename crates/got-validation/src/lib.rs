pub struct CrossValidator;

impl CrossValidator {
    pub fn validate_synthetic_claims(evidence_kind: &str, claims_inferential: bool) -> bool {
        if evidence_kind == "synthetic" && claims_inferential {
            false
        } else {
            true
        }
    }
}
