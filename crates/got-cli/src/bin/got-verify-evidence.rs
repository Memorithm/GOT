//! Offline, Docker-free structural and hash audit of a previously recorded
//! external-agent OCI campaign. This does not authenticate the report signer
//! or confer permission to make inferential claims about an AI model.

use got_runner::external::ExternalCampaign;
use got_validation::external::ExternalEvidenceVerifier;
use std::path::Path;

fn run(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("evidence manifest not accessible: {e}"))?;
    if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
        return Err("evidence must be a regular JSON file of at most 32 MiB".into());
    }
    let raw = std::fs::read(path).map_err(|e| format!("unable to read evidence: {e}"))?;
    let report: ExternalCampaign = serde_json::from_slice(&raw)
        .map_err(|e| format!("invalid evidence schema: {e}"))?;
    let verification = ExternalEvidenceVerifier::verify(&report)?;
    serde_json::to_string_pretty(&verification)
        .map_err(|e| format!("cannot serialize audit summary: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        println!("Usage: got-verify-evidence <external-report.json>");
        println!("Verifies frame SHA-256, expected responses, scores and paired identities without Docker.");
        println!("This verifies internal consistency, not the identity of the model or origin host.");
        return;
    }
    if args.len() != 2 {
        eprintln!("Usage: got-verify-evidence <external-report.json>");
        std::process::exit(2);
    }
    match run(Path::new(&args[1])) {
        Ok(out) => println!("{out}"),
        Err(e) => {
            eprintln!("GOT evidence audit rejected: {e}");
            std::process::exit(2);
        }
    }
}
