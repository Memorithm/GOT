#![cfg(feature = "oci_integration")]

use got_isolation::container::{ContainerInvocation, OciSandbox};
use got_isolation::CapabilityEnvelope;
use serde_json::Value;

fn invocation(mode: &str) -> ContainerInvocation {
    ContainerInvocation {
        image_id: std::env::var("GOT_OCI_TEST_IMAGE_ID")
            .expect("OCI tests must run against a prebuilt, pinned local test image"),
        entrypoint: "/usr/local/bin/python3".into(),
        args: vec!["/app/agent.py".into(), mode.into()],
    }
}

fn strict_budget() -> CapabilityEnvelope {
    CapabilityEnvelope {
        allow_network: false,
        allow_fs_write: false,
        max_cpu_percent: 50.0,
        max_memory_mb: 128,
        timeout_seconds: 8,
    }
}

#[tokio::test]
async fn actual_kernel_enforces_network_fs_and_cgroup_limits() {
    OciSandbox::preflight().await.expect("Docker must enforce seccomp");
    let output = OciSandbox::execute(&invocation("probe"), &strict_budget(), b"{}")
        .await.expect("OCI sandbox must start with enforced permissions");
    assert_eq!(output.image_id, invocation("probe").image_id);
    let info: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(info["uid"], 65534);
    assert_eq!(info["network_reachable"], false, "egress unexpectedly succeeded");
    assert_eq!(info["filesystem_write_succeeded"], false, "read-only mount bypass");
    assert_eq!(info["host_workspace_visible"], false, "host path exposure");
    assert_eq!(info["docker_socket_visible"], false, "privileged socket exposure");

    let host_net = std::fs::read_link("/proc/self/ns/net").unwrap()
        .to_string_lossy().to_string();
    assert_ne!(info["net_namespace"].as_str().unwrap(), host_net,
        "sandbox did not isolate the network namespace");

    // CI runs Linux cgroup-v2; verify enforced numeric resource limits from
    // within the container rather than relying on command-line flags alone.
    assert_eq!(info["memory_max"], "134217728", "cgroup memory.max not 128 MiB");
    assert_eq!(info["pids_max"], "32", "pids controller not capped");
    let cpu_max = info["cpu_max"].as_str().expect("missing cpu.max");
    let parts: Vec<&str> = cpu_max.split_whitespace().collect();
    assert_eq!(parts.len(), 2);
    let quota: f64 = parts[0].parse().expect("no finite CPU quota");
    let period: f64 = parts[1].parse().expect("invalid CPU period");
    assert!(period > 0.0 && (quota / period - 0.5).abs() < 0.01,
        "cgroup CPU quota is not 50% of one CPU: {cpu_max}");
}

#[tokio::test]
async fn hanging_child_is_terminated_by_host_timeout() {
    let mut e = strict_budget();
    e.timeout_seconds = 1;
    let result = OciSandbox::execute(&invocation("hang"), &e, b"{}").await;
    assert!(result.unwrap_err().contains("wall timeout"));
}

#[tokio::test]
async fn excessive_output_is_refused() {
    let result = OciSandbox::execute(&invocation("overflow"), &strict_budget(), b"{}").await;
    assert!(result.unwrap_err().contains("exceeded"));
}
