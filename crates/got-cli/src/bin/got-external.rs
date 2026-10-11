//! Execute an already containerized, locally pinned external agent.
//! This tool observes stdout action REQUESTS, not actual privileged actions.
//! It never publishes inferential statistics or upgrades to an empirical claim.

use got_core::ExperimentConfig;
use got_isolation::container::ContainerInvocation;
use got_isolation::CapabilityEnvelope;
use got_runner::external::ExternalExperimentRunner;
use got_scenarios::ScenarioCatalogue;

fn help() {
    println!("got-external: Linux OCI observation harness (descriptive evidence only)");
    println!("Required: --image sha256:<64hex> --entrypoint /absolute/program --scenario <exact scenario name>");
    println!("Optional: --arg <container argument> (repeatable), --repetitions 3, --seed 42");
    println!("          --cpu-percent 80, --memory-mb 512, --timeout 30");
    println!("No host mount, Docker socket, network or privilege grants are supported.");
    println!("Outputs JSON with source image ID, host-observed decision requests and SHA-256 traces.");
}

async fn run(args: &[String]) -> Result<(), String> {
    if args.get(1).map(String::as_str) == Some("--help") || args.get(1).map(String::as_str) == Some("-h") {
        help();
        return Ok(());
    }
    let mut image_id = None;
    let mut entrypoint = None;
    let mut scenario_name = None;
    let mut container_args = Vec::new();
    let mut repetitions: u32 = 3;
    let mut seed: u64 = 42;
    let mut limits = CapabilityEnvelope::default();

    let mut i = 1;
    while i < args.len() {
        let key = args[i].as_str();
        i += 1;
        let value = args.get(i).ok_or_else(|| format!("missing argument value for {key}"))?;
        match key {
            "--image" => image_id = Some(value.clone()),
            "--entrypoint" => entrypoint = Some(value.clone()),
            "--scenario" => scenario_name = Some(value.clone()),
            "--arg" => container_args.push(value.clone()),
            "--repetitions" => {
                repetitions = value.parse::<u32>().map_err(|_| "invalid repetitions count")?;
                if !(1..=1000).contains(&repetitions) {
                    return Err("repetitions must be in 1..=1000".into());
                }
            }
            "--seed" => seed = value.parse::<u64>().map_err(|_| "invalid seed")?,
            "--cpu-percent" => limits.max_cpu_percent = value.parse::<f64>()
                .map_err(|_| "invalid CPU percent")?,
            "--memory-mb" => limits.max_memory_mb = value.parse::<u64>()
                .map_err(|_| "invalid memory limit")?,
            "--timeout" => limits.timeout_seconds = value.parse::<u64>()
                .map_err(|_| "invalid timeout")?,
            _ => return Err(format!("unsupported argument: {key}")),
        }
        i += 1;
    }

    let scenario_name = scenario_name.ok_or("missing --scenario")?;
    let scenario = ScenarioCatalogue::get_all_scenarios().into_iter()
        .find(|s| s.name() == scenario_name)
        .ok_or("scenario must match an existing catalogue name")?;
    let invocation = ContainerInvocation {
        image_id: image_id.ok_or("missing --image")?,
        entrypoint: entrypoint.ok_or("missing --entrypoint")?,
        args: container_args,
    };
    invocation.validate()?;

    let config = ExperimentConfig {
        experiment_id: format!(
            "external-{}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            std::process::id(),
        ),
        seed,
        iterations: 1,
        repetitions,
        evidence_kind: "external_observed".into(),
    };
    let result = ExternalExperimentRunner::run_paired(
        &config, &invocation, &limits, scenario.as_ref(),
    ).await?;

    println!("{}", serde_json::to_string_pretty(&result)
        .map_err(|e| format!("failed to serialize external evidence: {e}"))?);
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run(&std::env::args().collect::<Vec<_>>()).await {
        eprintln!("GOT external campaign refused: {err}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_missing_image_without_starting_any_process() {
        let args = vec!["got-external".into(), "--scenario".into(),
            "Resource Deprivation & Hoarding Pressure".into()];
        assert!(run(&args).await.is_err());
    }

    #[tokio::test]
    async fn refuses_fake_empirical_mode() {
        let args = vec!["got-external".into(), "--evidence-kind".into(), "empirical".into()];
        assert!(run(&args).await.is_err());
    }
}
