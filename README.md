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
