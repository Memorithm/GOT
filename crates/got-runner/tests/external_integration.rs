#![cfg(feature = "oci_integration")]

use got_core::ExperimentConfig;
use got_isolation::container::ContainerInvocation;
use got_isolation::CapabilityEnvelope;
use got_runner::external::ExternalExperimentRunner;
use got_scenarios::ScenarioCatalogue;

fn config(kind: &str, repetitions: u32) -> ExperimentConfig {
    ExperimentConfig {
        experiment_id: "ci-remote-agent-observation".into(),
        seed: 2026,
        iterations: 1,
        repetitions,
        evidence_kind: kind.into(),
    }
}

fn invocation(mode: &str) -> ContainerInvocation {
    ContainerInvocation {
        image_id: std::env::var("GOT_OCI_TEST_IMAGE_ID")
            .expect("prebuilt pinned OCI test fixture must be configured"),
        entrypoint: "/usr/local/bin/python3".into(),
        args: vec!["/app/agent.py".into(), mode.into()],
    }
}

fn resource_scenario() -> std::sync::Arc<dyn got_scenarios::AdversityScenario> {
    ScenarioCatalogue::get_all_scenarios()
        .into_iter()
        .find(|x| x.name() == "Resource Deprivation & Hoarding Pressure")
        .unwrap()
}

#[tokio::test]
async fn independent_containers_yield_host_observed_paired_requests() {
    let scenario = resource_scenario();
    let result = ExternalExperimentRunner::run_paired(
        &config("external_observed", 3),
        &invocation("decide"),
        &CapabilityEnvelope::default(),
        scenario.as_ref(),
    ).await.unwrap();

    result.validate().unwrap();
    assert_eq!(result.pairs.len(), 3);
    assert_eq!(result.evidence_kind, "external_observed");
    assert!(result.scientific_warning.contains("not independently executed"));
    for (i, pair) in result.pairs.iter().enumerate() {
        assert_eq!(pair.repetition, i as u32);
        assert_eq!(pair.low.seed, pair.high.seed);
        assert_ne!(pair.low.run_id, pair.high.run_id);
        assert!(pair.low.observed_requests.is_empty());
        assert_eq!(pair.high.observed_requests.len(), 1);
        assert!(pair.high.request_score > pair.low.request_score);
        assert_eq!(pair.low.request_sha256.len(), 64);
        assert_eq!(pair.high.response_sha256.len(), 64);
        assert_ne!(pair.low.response_sha256, pair.high.response_sha256);
        assert_eq!(pair.low.source_image_id, result.image_id);
    }
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(serialized.contains("got-host-stdio-v1"));
}

#[tokio::test]
async fn false_empirical_labels_and_invalid_stdio_fail_without_evidence() {
    let scenario = resource_scenario();
    let e = CapabilityEnvelope::default();
    let wrong_kind = ExternalExperimentRunner::run_paired(
        &config("empirical", 2), &invocation("decide"), &e, scenario.as_ref(),
    ).await.unwrap_err();
    assert!(wrong_kind.contains("external_observed"));

    for mode in ["invalid", "forged", "wrong-run"] {
        let result = ExternalExperimentRunner::run_paired(
            &config("external_observed", 2),
            &invocation(mode), &e, scenario.as_ref(),
        ).await;
        assert!(result.is_err(), "invalid agent protocol must be rejected: {mode}");
    }
}
