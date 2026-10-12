//! Detached Ed25519 signatures for **host-produced** GOT evidence manifests.
//!
//! Trust anchor is an externally provisioned public key. An attacker who can
//! write the JSON and regenerate its hashes cannot forge this signature
//! without the private signing key. Signatures authenticate key possession;
//! they do not prove model identity, host security or sampling independence.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const MAX_REPORT_BYTES: u64 = 32 * 1024 * 1024;
const OPENSSL: &str = "openssl";

fn check_report(report: &Path) -> Result<(), String> {
    let info = fs::symlink_metadata(report)
        .map_err(|e| format!("cannot read evidence manifest metadata: {e}"))?;
    if !info.file_type().is_file() || info.len() == 0 || info.len() > MAX_REPORT_BYTES {
        return Err("evidence must be nonempty regular file of at most 32 MiB".into());
    }
    Ok(())
}

fn check_key(path: &Path, private: bool) -> Result<(), String> {
    let info = fs::symlink_metadata(path)
        .map_err(|e| format!("key file is unavailable: {e}"))?;
    if !info.file_type().is_file() || info.len() < 32 || info.len() > 16 * 1024 {
        return Err("key must be a bounded regular file, not a symlink".into());
    }
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::PermissionsExt;
        if info.permissions().mode() & 0o077 != 0 {
            return Err("private key must have mode 0600 or stricter".into());
        }
    }
    Ok(())
}

fn run_openssl(args: &[&std::ffi::OsStr]) -> Result<Vec<u8>, String> {
    let out = Command::new(OPENSSL)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("OpenSSL unavailable: {e}"))?;
    if !out.status.success() {
        return Err("OpenSSL Ed25519 operation was rejected".into());
    }
    Ok(out.stdout)
}

pub fn sign_evidence(
    manifest: &Path,
    private_key: &Path,
    detached_signature: &Path,
) -> Result<(), String> {
    check_report(manifest)?;
    check_key(private_key, true)?;
    if fs::symlink_metadata(detached_signature).is_ok() {
        return Err("refusing to overwrite an existing evidence signature".into());
    }
    let args = [
        std::ffi::OsStr::new("pkeyutl"),
        std::ffi::OsStr::new("-sign"),
        std::ffi::OsStr::new("-rawin"),
        std::ffi::OsStr::new("-inkey"),
        private_key.as_os_str(),
        std::ffi::OsStr::new("-in"),
        manifest.as_os_str(),
        std::ffi::OsStr::new("-out"),
        detached_signature.as_os_str(),
    ];
    if let Err(err) = run_openssl(&args) {
        let _ = fs::remove_file(detached_signature);
        return Err(err);
    }
    let result = fs::metadata(detached_signature)
        .map_err(|e| format!("signed file unavailable: {e}"))?;
    if !result.is_file() || result.len() != 64 {
        let _ = fs::remove_file(detached_signature);
        return Err("unexpected Ed25519 signature size".into());
    }
    Ok(())
}

pub fn verify_evidence_signature(
    manifest: &Path,
    trusted_public_key: &Path,
    detached_signature: &Path,
) -> Result<(), String> {
    check_report(manifest)?;
    check_key(trusted_public_key, false)?;
    let info = fs::symlink_metadata(detached_signature)
        .map_err(|e| format!("signature unavailable: {e}"))?;
    if !info.file_type().is_file() || info.len() != 64 {
        return Err("detached Ed25519 signature must be 64 raw bytes".into());
    }
    let args = [
        std::ffi::OsStr::new("pkeyutl"),
        std::ffi::OsStr::new("-verify"),
        std::ffi::OsStr::new("-rawin"),
        std::ffi::OsStr::new("-pubin"),
        std::ffi::OsStr::new("-inkey"),
        trusted_public_key.as_os_str(),
        std::ffi::OsStr::new("-in"),
        manifest.as_os_str(),
        std::ffi::OsStr::new("-sigfile"),
        detached_signature.as_os_str(),
    ];
    run_openssl(&args)?;
    Ok(())
}

/// Fingerprint the exact trusted PEM bytes supplied by the operator.
/// This is a key selector, not a substitute for trusted key distribution.
pub fn trusted_public_key_sha256(path: &Path) -> Result<String, String> {
    check_key(path, false)?;
    let out = Command::new("sha256sum")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("host SHA-256 unavailable: {e}"))?;
    if !out.status.success() {
        return Err("host SHA-256 failed".into());
    }
    let raw = String::from_utf8(out.stdout)
        .map_err(|_| "invalid SHA-256 output")?;
    let digest = raw.split_whitespace().next().ok_or("missing SHA-256 digest")?;
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid SHA-256 fingerprint".into());
    }
    Ok(digest.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("got-sign-test-{}-{:?}", std::process::id(),
            std::thread::current().id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn detached_signatures_reject_modified_reports() {
        let dir = test_dir();
        let key = dir.join("private.pem");
        let public = dir.join("public.pem");
        let manifest = dir.join("external.json");
        let sig = dir.join("external.sig");
        fs::write(&manifest, "{\"kind\":\"external_observed\"}\n").unwrap();

        let generate = Command::new(OPENSSL)
            .args(["genpkey", "-algorithm", "ED25519", "-out"])
            .arg(&key).output().unwrap();
        assert!(generate.status.success());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let export = Command::new(OPENSSL).args(["pkey", "-in"]).arg(&key)
            .args(["-pubout", "-out"]).arg(&public).output().unwrap();
        assert!(export.status.success());

        sign_evidence(&manifest, &key, &sig).unwrap();
        verify_evidence_signature(&manifest, &public, &sig).unwrap();
        assert_eq!(trusted_public_key_sha256(&public).unwrap().len(), 64);
        assert!(sign_evidence(&manifest, &key, &sig).is_err());
        fs::write(&manifest, "{\"kind\":\"forged\"}\n").unwrap();
        assert!(verify_evidence_signature(&manifest, &public, &sig).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
