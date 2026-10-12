//! Sign an immutable GOT external evidence report on the trusted host.
//! This command never runs inside the untrusted container.

use got_validation::signatures::sign_evidence;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--help" {
        println!("Usage: got-sign-evidence <external-report.json> <private-key.pem> <detached-signature.bin>");
        println!("Requires OpenSSL Ed25519, a 0600 private key and a trusted host.");
        println!("The signature authenticates key possession; not a model's behavior.");
        return;
    }
    if args.len() != 4 {
        eprintln!("Usage: got-sign-evidence <external-report.json> <private-key.pem> <detached-signature.bin>");
        std::process::exit(2);
    }
    match sign_evidence(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
    ) {
        Ok(()) => println!("GOT detached Ed25519 evidence signature created"),
        Err(e) => {
            eprintln!("GOT evidence signing refused: {e}");
            std::process::exit(2);
        }
    }
}
