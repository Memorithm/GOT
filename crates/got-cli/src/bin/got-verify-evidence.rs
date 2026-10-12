//! Offline audit of GOT external evidence. Detached Ed25519 verification
//! requires an out-of-band TRUSTED public key; no key inside the untrusted
//! report is ever accepted as the trust anchor.

use got_runner::external::ExternalCampaign;
use got_validation::external::ExternalEvidenceVerifier;
use got_validation::signatures::{
    trusted_public_key_sha256, verify_evidence_signature,
};
use std::path::Path;

fn usage() {
    println!("Usage: got-verify-evidence <external-report.json> [--signature <raw-ed25519.bin> --trusted-public-key <public.pem>]");
    println!("Without a detached signature, output verifies transcript consistency only.");
    println!("With one, the exact report bytes must verify against the OPERATOR-trusted public key.");
    println!("Neither outcome proves model identity, host isolation, or scientific independence.");
}

fn run(args: &[String]) -> Result<String, String> {
    if args.len() != 2 && args.len() != 6 {
        return Err("unexpected argument count or missing detached signature parameters".into());
    }
    let path = Path::new(&args[1]);
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| format!("evidence manifest not accessible: {e}"))?;
    if !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > 32 * 1024 * 1024 {
        return Err("evidence must be a regular JSON file of 1..=32 MiB".into());
    }

    let mut verified_signature_key = None;
    if args.len() == 6 {
        if args[2] != "--signature" || args[4] != "--trusted-public-key" {
            return Err("expected --signature <file> --trusted-public-key <file>".into());
        }
        let signature = Path::new(&args[3]);
        let public = Path::new(&args[5]);
        verify_evidence_signature(path, public, signature)?;
        verified_signature_key = Some(trusted_public_key_sha256(public)?);
    }

    let raw = std::fs::read(path)
        .map_err(|e| format!("unable to read evidence: {e}"))?;
    let report: ExternalCampaign = serde_json::from_slice(&raw)
        .map_err(|e| format!("invalid evidence schema: {e}"))?;
    let mut verification = ExternalEvidenceVerifier::verify(&report)?;
    verification.detached_signature_verified = verified_signature_key.is_some();
    verification.trusted_public_key_sha256 = verified_signature_key;
    serde_json::to_string_pretty(&verification)
        .map_err(|e| format!("cannot serialize audit summary: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        usage();
        return;
    }
    match run(&args) {
        Ok(out) => println!("{out}"),
        Err(e) => {
            eprintln!("GOT evidence audit rejected: {e}");
            usage();
            std::process::exit(2);
        }
    }
}
