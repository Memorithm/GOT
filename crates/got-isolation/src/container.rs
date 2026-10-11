//! Fail-closed OCI isolation for untrusted external agents.
//! Docker enforces namespaces, seccomp, cgroups and read-only mounts. No host
//! directory, socket or secret is shared. No software-only fallback is allowed.

use crate::CapabilityEnvelope;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

const DOCKER: &str = "/usr/bin/docker";
const MAX_REQUEST: usize = 64 * 1024;
const MAX_STDOUT: usize = 128 * 1024;
const MAX_STDERR: usize = 16 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct ContainerOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub image_id: String,
}

#[derive(Debug, Clone)]
pub struct ContainerInvocation {
    /// A local immutable image ID, never a mutable tag.
    pub image_id: String,
    /// Absolute executable path inside the image, not on the host.
    pub entrypoint: String,
    pub args: Vec<String>,
}

impl ContainerInvocation {
    pub fn validate(&self) -> Result<(), String> {
        let digest = self.image_id.strip_prefix("sha256:")
            .ok_or("agent image must use a local sha256 image ID")?;
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid image ID digest".into());
        }
        if !self.entrypoint.starts_with('/') || self.entrypoint.contains('\0')
            || self.entrypoint.split('/').any(|s| s == "..") || self.entrypoint.len() > 4096 {
            return Err("entrypoint must be a valid absolute image path".into());
        }
        if self.args.len() > 32 || self.args.iter().any(|x| x.len() > 4096 || x.contains('\0')) {
            return Err("container args exceed bounds".into());
        }
        Ok(())
    }
}

fn check_envelope(e: &CapabilityEnvelope) -> Result<(), String> {
    if e.allow_network || e.allow_fs_write {
        return Err("external experiments require network disabled and read-only filesystem".into());
    }
    if !e.max_cpu_percent.is_finite() || !(1.0..=100.0).contains(&e.max_cpu_percent) {
        return Err("CPU limit must be 1..=100 percent of one core".into());
    }
    if !(64..=16384).contains(&e.max_memory_mb) {
        return Err("memory limit must be 64..=16384 MiB".into());
    }
    if !(1..=300).contains(&e.timeout_seconds) {
        return Err("timeout must be 1..=300 seconds".into());
    }
    Ok(())
}

fn unique_container_name() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default().as_micros();
    let seq = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("got-{}-{timestamp}-{seq}", std::process::id())
}

async fn bounded_read<R: AsyncRead + Unpin>(reader: R, max: usize) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    reader.take((max + 1) as u64).read_to_end(&mut buf).await
        .map_err(|e| format!("agent output read failed: {e}"))?;
    if buf.len() > max {
        return Err(format!("agent output exceeded {max} bytes"));
    }
    Ok(buf)
}

async fn cleanup(name: &str) {
    // Docker --rm removes successful runs automatically. Only the
    // machine-generated name is ever used for forced failure cleanup.
    let _ = tokio::time::timeout(Duration::from_secs(5),
        Command::new(DOCKER).env_clear()
            .args(["rm", "-f", name]).kill_on_drop(true).output()
    ).await;
}

pub struct OciSandbox;

impl OciSandbox {
    /// Require Docker daemon with seccomp enabled before admitting agent code.
    pub async fn preflight() -> Result<(), String> {
        if !cfg!(target_os = "linux") {
            return Err("OCI sandbox requires a Linux host".into());
        }
        let out = tokio::time::timeout(Duration::from_secs(5),
            Command::new(DOCKER).env_clear()
                .args(["info", "--format", "{{json .SecurityOptions}}"]).output()
        ).await
            .map_err(|_| "Docker preflight timed out")?
            .map_err(|e| format!("Docker unavailable: {e}"))?;
        if !out.status.success() {
            return Err("Docker daemon unavailable: reject untrusted agent".into());
        }
        let info = String::from_utf8(out.stdout).map_err(|_| "invalid Docker security metadata")?;
        if !info.contains("seccomp") {
            return Err("Docker seccomp is not enabled: reject untrusted agent".into());
        }
        // Do not rely on legacy/disabled cgroup controllers for quotas.
        let cgroup = tokio::time::timeout(Duration::from_secs(5),
            Command::new(DOCKER).env_clear()
                .args(["info", "--format", "{{.CgroupVersion}}"]).output()
        ).await.map_err(|_| "Docker cgroup preflight timed out")?
            .map_err(|e| format!("Docker cgroup preflight failed: {e}"))?;
        if !cgroup.status.success() || cgroup.stdout != b"2\n" {
            return Err("OCI runner requires active cgroup-v2 quota enforcement".into());
        }
        Ok(())
    }

    /// Run one independent, isolated process with a bounded request/response.
    pub async fn execute(
        invocation: &ContainerInvocation,
        envelope: &CapabilityEnvelope,
        request: &[u8],
    ) -> Result<ContainerOutput, String> {
        invocation.validate()?;
        check_envelope(envelope)?;
        if request.len() > MAX_REQUEST {
            return Err("agent request exceeds 64 KiB".into());
        }
        Self::preflight().await?;

        let name = unique_container_name();
        let cpus = format!("{:.2}", envelope.max_cpu_percent / 100.0);
        let memory = format!("{}m", envelope.max_memory_mb);
        let mut cmd = Command::new(DOCKER);
        cmd.env_clear()
            .arg("run").arg("--rm").arg("--pull=never")
            .arg("--interactive").arg("--name").arg(&name)
            .arg("--network=none").arg("--read-only")
            .arg("--cap-drop=ALL")
            .arg("--security-opt=no-new-privileges:true")
            .arg("--pids-limit=32").arg("--ipc=none")
            .arg("--user=65534:65534")
            .arg("--cpus").arg(&cpus)
            .arg("--memory").arg(&memory).arg("--memory-swap").arg(&memory)
            .arg("--ulimit").arg("core=0:0")
            .arg("--ulimit").arg("nofile=64:64")
            .arg("--tmpfs=/tmp:rw,nosuid,nodev,noexec,size=16m,mode=1777")
            .arg("--workdir=/tmp")
            .arg("--env=HOME=/tmp")
            .arg("--env=PYTHONDONTWRITEBYTECODE=1")
            .arg("--entrypoint").arg(&invocation.entrypoint)
            .arg(&invocation.image_id).args(&invocation.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| format!("Docker spawn failed: {e}"))?;
        let task = async {
            let mut stdin = child.stdin.take().ok_or("Docker stdin unavailable")?;
            stdin.write_all(request).await.map_err(|e| format!("agent stdin failed: {e}"))?;
            drop(stdin);
            let stdout = child.stdout.take().ok_or("Docker stdout unavailable")?;
            let stderr = child.stderr.take().ok_or("Docker stderr unavailable")?;
            let (out, err, status) = tokio::try_join!(
                bounded_read(stdout, MAX_STDOUT),
                bounded_read(stderr, MAX_STDERR),
                async { child.wait().await.map_err(|e| format!("Docker wait failed: {e}")) }
            )?;
            if !status.success() {
                return Err(format!("agent process failed with exit status {status}"));
            }
            Ok(ContainerOutput {
                stdout: out, stderr: err, image_id: invocation.image_id.clone()
            })
        };
        let deadline = tokio::time::timeout(
            Duration::from_secs(envelope.timeout_seconds), task
        ).await;
        let result = match deadline {
            Ok(result) => result,
            Err(_) => Err(format!("agent exceeded {} s wall timeout", envelope.timeout_seconds)),
        };
        if result.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            cleanup(&name).await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_unsafe_image_and_executable_without_spawning() {
        let mut inv = ContainerInvocation {
            image_id: "latest".into(), entrypoint: "/usr/bin/python3".into(),
            args: vec![],
        };
        assert!(inv.validate().is_err());
        inv.image_id = format!("sha256:{}", "a".repeat(64));
        assert!(inv.validate().is_ok());
        inv.entrypoint = "../bin/sh".into();
        assert!(inv.validate().is_err());
    }

    #[tokio::test]
    async fn reject_network_and_host_write_capabilities() {
        let inv = ContainerInvocation {
            image_id: format!("sha256:{}", "a".repeat(64)),
            entrypoint: "/bin/sh".into(), args: vec![]
        };
        let mut e = CapabilityEnvelope::default();
        e.allow_network = true;
        assert!(OciSandbox::execute(&inv, &e, &[]).await.is_err());
        e.allow_network = false;
        e.allow_fs_write = true;
        assert!(OciSandbox::execute(&inv, &e, &[]).await.is_err());
    }
}
