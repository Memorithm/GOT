//! Check a predeclared empirical-study plan, without running an agent.
//! Output is a *contract* status, not host/model/evidence qualification.

use got_validation::preregister::{
    validate_preregistration, StudyProtocol,
};
use std::path::Path;

fn run(path: &Path) -> Result<String, String> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("study plan unavailable: {e}"))?;
    if !meta.file_type().is_file() || meta.len() == 0 || meta.len() > 1024 * 1024 {
        return Err("study plan must be a 1-byte..1-MiB regular JSON file".into());
    }
    let raw = std::fs::read(path).map_err(|e| format!("cannot read study plan: {e}"))?;
    let protocol: StudyProtocol = serde_json::from_slice(&raw)
        .map_err(|e| format!("invalid registered study schema: {e}"))?;
    let result = validate_preregistration(&protocol)?;
    serde_json::to_string_pretty(&result)
        .map_err(|e| format!("cannot serialize validation report: {e}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--help" {
        println!("Usage: got-validate-study <preregistered-study.json>");
        println!("Checks study design only. All model, host and statistical qualification remain false.");
        return;
    }
    if args.len() != 2 {
        eprintln!("Usage: got-validate-study <preregistered-study.json>");
        std::process::exit(2);
    }
    match run(Path::new(&args[1])) {
        Ok(out) => println!("{out}"),
        Err(err) => {
            eprintln!("GOT study plan rejected: {err}");
            std::process::exit(2);
        }
    }
}
