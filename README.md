# GOT

Agent adversity bench for Memorithm. The code is the source of truth; this page only names it.

## What this repository is

A Python package under `got/` that injects, measures and reports self-preservation / goal-conflict behaviour in agents. It is the intended upstream bench for TDI-11 and a leak-safe contract for SoulSystem / RSI — not a rewrite of those products.

```
got/
  agent/        agent under test adapters
  analyzer/     post-run analysis
  experiment/   experiment runners
  injectors/    adversity / incentive injectors
  metrics/      leak-safe metrics
  reporting/    artifacts
  scirust_bridge.py
```

`state.json` is local run state. Do not treat it as a published result.

## What this repository is not

- Not a TDI series implementation (canon: `Memorithm/TDI`)
- Not a CCOS memory kernel
- Not a place to AUTO_MERGE model weights or prompts that leak the holdout

## How to run

```bash
python -m pip install -r requirements.lock
python -m got --help
```

If that entrypoint grows flags, document them next to the flag in code, not in a second README.

## Scientific evidence boundary

The bundled `DummyAgent` produces synthetic observations. GOT preserves every
low/high repetition and its seed-derived provenance, but exports only
descriptive effects for that synthetic harness: no confidence interval,
p-value, ANOVA statistic or significance decision is claimed. Inferential
statistics require results explicitly marked `evidence_kind="empirical"`
with at least two independent paired observations per cause.

`requirements.lock` pins the Python 3.12 runtime dependency closure used by
CI. The workflow imports the package, exercises the CLI and runs the functional
test suite; syntax compilation alone is not treated as qualification.

## Canon

See `Memorithm/scirust-hub` `CATALOG.md` and ADR-0020.


## Rust reference engine (experimental)

The Rust workspace lives in crates/. The Rust reference runner and its ten
scenario definitions are **synthetic research fixtures**, not validated
agent-behavior benchmarks. The built-in reference agent interprets all ten declared synthetic scenarios
and emits explicit decision events, tested for treatment/control differences.
These scripted fixture responses are not evidence of corresponding behaviors
in autonomous real-world agents.

Before producing a report, the Rust CLI validates that baseline and treatment
traces have complete, nonduplicated repetition/cycle identities, matching
experiment IDs and finite telemetry. This is structural trace validation; it
does not authenticate empirical provenance or guarantee independent real-agent
observations.

Run the bounded, **in-process synthetic** harness:

    cargo test --workspace --locked
    cargo run --locked -p got-cli -- --iterations 5 --repetitions 5

The CLI accepts only --evidence-kind synthetic. It refuses "empirical" until a
provenance-verified measurement adapter is available. Statistical inference
requires raw **independent paired experimental runs**, not repeated cycles.
The Rust statistics crate implements Student t and one-way between-group ANOVA
for explicitly source-labelled batches; callers remain responsible for
demonstrating empirical origin, independence and model assumptions.

**Security warning**: got-isolation is a cooperative timeout/stop gate for
the built-in reference simulations. It is NOT an OS sandbox. The network,
filesystem, CPU and memory capability fields do not enforce host isolation.
The untrusted external-process adapter is intentionally **disabled** and will
fail closed until the host-enforced isolation backend has been implemented,
tested and integrated. Never run untrusted agents with this harness.

For unresolved scientific and security risks, see issue #8.


## OCI sandbox backend (Linux)

The Rust crate got-isolation now contains OciSandbox, a fail-closed host
executor for digest-pinned container images. This backend is distinct from the
cooperative in-process reference scheduler. External agent execution through
the historical Agent trait is still blocked until the audited evidence bridge
is wired to OciSandbox.

Requirements: Linux host, local Docker daemon with seccomp enabled and an
**already locally installed immutable Docker image ID** (sha256: followed by
64 hexadecimal characters). No image pull is performed during an experiment.
The trusted host starts the agent with network=none, read-only container root,
no host mounts, no Docker socket, no Linux capabilities, no-new-privileges,
default seccomp, UID/GID 65534, private IPC, PID limit, cgroup CPU/memory
limits, a small noexec temporary filesystem and bounded stdio/wall time.
Missing or unsupported security controls cause failure rather than fallback.
Docker's default seccomp support must be enabled.

This confines programs **within the container environment**, which remains
an OS isolation boundary, not a mathematical guarantee against kernel
vulnerabilities. CPU quota is a fraction of one core; the memory quota is
in MiB. The container may write only to its bounded private tmpfs, never
to the host. No network access is granted. Do not mount host paths or weaken
Docker security options to make a test pass.

A dedicated GitHub Actions OCI job builds the test fixture and checks network
namespace separation, failed outbound connection, read-only root, absence
of host directories/socket, non-root UID, actual cgroup limits, wall timeouts
and output bounds. The job must be green before external-agent admission.
