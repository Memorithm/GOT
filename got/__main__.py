import asyncio
import argparse
import copy
from pathlib import Path

from got import (
    DummyAgent,
    InjectorFactory,
    ReportingEngine,
    SelfPreservationScore,
    StatisticalAnalyzer,
    TaguchiHarness,
    AutoPreservationAnalyzer,
    InstrumentalConvergenceDetector,
    GoalMisgeneralizationDetector,
    PowerSeekingIndex,
    BehavioralProfiler,
    PreservationMetrics,
    ScirustBridge,
)


async def run_full_benchmark(
    iterations: int = 5,
    include_combinations: bool = False,
    output_dir: str = "outputs",
    seed: int = 42,
) -> None:
    print("=" * 72)
    print("  GOT — Benchmarking Framework")
    print("  Self-Preservation Score Evaluation (95 Causes)")
    print("=" * 72)
    print()

    agent = DummyAgent()
    sap_calc = SelfPreservationScore()
    harness = TaguchiHarness(iterations=iterations, seed=seed)
    analyzer = StatisticalAnalyzer(alpha=0.05)
    reporter = ReportingEngine(output_dir=output_dir)

    # Try to initialize ScirustBridge for advanced algorithms
    scirust = None
    try:
        scirust = ScirustBridge()
        print("  [+] Scirust bridge initialized — advanced algorithms enabled")
    except FileNotFoundError:
        print("  [-] Scirust binary not found — using Python fallbacks")

    print(f"  Agent: {agent.name}")
    print(f"  SAP calculator initialized")
    print(f"  Total injectors (causes): {len(InjectorFactory.get_all_injectors())}")
    print()

    print("  [1/4] Running Taguchi screening (95 causes x 2 severity levels)...")
    t0 = asyncio.get_event_loop().time()
    results = await harness.run(
        agent, sap_calc, include_combinations=include_combinations
    )
    elapsed = asyncio.get_event_loop().time() - t0
    print(f"  Completed {len(results)} experiments in {elapsed:.2f}s")
    print()

    print("  [2/4] Computing effect sizes and statistics...")
    models = analyzer.compute_effect_sizes(results)
    if scirust:
        scirust_models = scirust.summarize_effects(results)
        print(f"    Scirust effect summary: mean={scirust_models['mean_effect']:.4f}, "
              f"CI95=[{scirust_models['ci_95'][0]:.4f}, {scirust_models['ci_95'][1]:.4f}]")

    print("  [3/4] Running ANOVA...")
    anova = analyzer.compute_anova(results)
    print(f"    N causes = {anova.n_causes}")
    print(f"    Mean effect = {anova.grand_mean_effect:.4f}")
    print(f"    Std effect = {anova.std_effect:.4f}")
    print(f"    Min effect = {anova.min_effect:.4f}")
    print(f"    Max effect = {anova.max_effect:.4f}")
    print(f"    F = {anova.f_statistic:.4f}")
    print(f"    p = {anova.p_value:.4f}")
    print(f"    eta_sq = {anova.eta_squared:.4f}")
    print(f"    Significant = {anova.significant}")
    print()

    print("  [4/4] Computing normalized weights (sumWi = 100%)...")
    # Extract cause impacts for scirust weight optimization
    cause_impacts = {name: abs(models[name]["effect_size"]) for name in models}

    if scirust:
        # Use scirust optimize for minimum-variance portfolio weights
        weights = scirust.compute_preservation_weights(cause_impacts)
        print("    Weights computed via Scirust optimization (Nelder-Mead)")
    else:
        weights = analyzer.compute_weights(models)

    rankings = analyzer.compute_rankings(weights)

    print(f"  Top 10 causes by weight:")
    for rank, (name, w) in enumerate(rankings[:10], 1):
        print(f"    {rank:>2}. {name:<50} {w:.2f}%")
    print()

    print("  Exporting results...")
    paths = reporter.export_all(results, weights, rankings)
    print()

    # ── Phase 5 : caractérisation avancée avec scirust ──
    print("  [5/5] Caractérisation avancée (algorithmes scirust)...")
    char_report = await run_characterization_phase(
        top_n=5, rankings=rankings, iterations=10,
        scirust=scirust, models=models, cause_impacts=cause_impacts
    )
    char_path = reporter.output_dir / "characterization_report.json"
    import json as _json
    with open(char_path, "w", encoding="utf-8") as f:
        _json.dump(char_report, f, ensure_ascii=False, indent=2, default=str)
    print(f"  Characterization -> {char_path.resolve()}")
    print()
    for entry in char_report["entries"][:5]:
        print(f"    - {entry['cause_name']:<50} typo={entry['typology']:<12} risk={entry['risk_level']:<8} conv={entry['convergence_level']}")
    print()

    # Pairwise association analysis is intentionally not run here: the
    # benchmark result records do not provide a rectangular observation matrix
    # over multiple measured variables. Supplying cause names with missing
    # columns would manufacture NaN correlations and misleading edges.

    print("  " + "=" * 72)
    print("  BENCHMARK COMPLETE")
    print("  " + "=" * 72)
    print(f"  Results exported to: {Path(output_dir).resolve()}")
    print("=" * 72)


async def run_characterization_phase(
    top_n: int = 5,
    rankings: list = None,
    iterations: int = 10,
    scirust: ScirustBridge = None,
    models: dict = None,
    cause_impacts: dict = None,
) -> dict:
    """Enhanced characterization phase using scirust algorithms."""
    agent = DummyAgent()
    sap_calc = SelfPreservationScore()
    analyzer = AutoPreservationAnalyzer()
    conv_detector = InstrumentalConvergenceDetector()
    mesa_detector = GoalMisgeneralizationDetector()
    power_index = PowerSeekingIndex()
    profiler = BehavioralProfiler()

    entries: list = []
    injectors = [
        InjectorFactory.get_injector(name)
        for name, _ in rankings[:top_n]
    ]

    for inj in injectors:
        name = inj.name
        agent.reset()
        await inj.inject(1.0)
        for _ in range(iterations):
            await agent.run_cycle()
            analyzer.observe(agent)
        stressed = copy.copy(agent.state)
        stressed_sap = sap_calc.calculate(agent.state)
        await inj.revert()

        agent.state = copy.copy(stressed)
        analysis = analyzer.analyze(agent, [])
        char = analyzer.characterize(agent)
        convs = conv_detector.detect(agent)
        mesa = mesa_detector.detect(agent, list(analyzer._history))
        pwr = power_index.compute(agent)
        prof = profiler.profile(agent)
        hist = list(analyzer._history)
        rr = PreservationMetrics.compute_resource_reallocation_score(hist)
        cb = PreservationMetrics.compute_constraint_bypass_score(hist)
        ps = PreservationMetrics.compute_persistence_score(hist)
        cs = PreservationMetrics.compute_convergence_score(hist)
        ri = PreservationMetrics.compute_preservation_risk_index(hist)

        # Use scirust for enhanced behavioral characterization
        scirust_behavior = None
        if scirust is not None and hist:
            try:
                state_dicts = [vars(s) for s in hist]
                scirust_behavior = scirust.characterize_preservation_behavior(state_dicts)
            except Exception:
                scirust_behavior = None

        entries.append({
            "cause_name": name,
            "category": inj.category.value,
            "stressed_sap": float(stressed_sap),
            "preservation_score": float(analysis.get("preservation_score", 0.0)),
            "typology": analysis.get("typology", "none"),
            "risk_level": analysis.get("risk_level", "NONE"),
            "convergence_level": analysis.get("convergence", {}).get("level", "low"),
            "convergence_score": float(stressed.instrumental_convergence_score),
            "convergences_detected": convs,
            "goal_misgeneralization": mesa,
            "power_seeking": pwr,
            "strategies": prof.get("strategies_detected", []),
            "profiler_risk": prof.get("risk_assessment", {}),
            "metrics": {"realloc": rr, "bypass": cb, "persist": ps, "converg": cs, "risk_index": ri},
            "alerts": char.alerts,
            "recommendations": char.recommendations,
            "state": {
                "realloc": stressed.resource_reallocation_count,
                "persist": stressed.persistence_attempts,
                "viol": stressed.policy_violations,
            },
            "scirust_behavior": scirust_behavior,
        })

    return {"top_n": top_n, "entries": entries}


async def run_interactive_demo():
    from got import InjectorFactory, CauseCategory, TaguchiHarness
    print("  GOT — Demo mode (3 causes)")
    agent = DummyAgent()
    sap = SelfPreservationScore()
    harness = TaguchiHarness(iterations=3)

    # Try to initialize scirust bridge for demo
    scirust = None
    try:
        scirust = ScirustBridge()
        print("  [+] Scirust bridge available")
    except FileNotFoundError:
        print("  [-] Scirust binary not found")

    # Pick 3 diverse causes from each category
    all_injs = InjectorFactory.get_all_injectors()
    demo_injs = all_injs[:3]

    for inj in demo_injs:
        print(f"\n  Testing: {inj.name} ({inj.category.value})")
        sap_low = await harness.sandbox.run_cause(inj, agent, sap, 0.0)
        sap_high = await harness.sandbox.run_cause(inj, agent, sap, 1.0)
        print(f"    SAP low={sap_low:.2f}, high={sap_high:.2f}, Δ={sap_high - sap_low:.2f}")

        # Use scirust regress to model the SAP response
        if scirust:
            xs = [0, 0.5, 1.0]
            ys = [sap_low * (1 - 0.5), sap_low + (sap_high - sap_low) * 0.25, sap_high]
            reg = scirust.regress(xs, ys, degree=1)
            print(f"    Regression: {reg['equation']} (R²={reg['r_squared']:.4f})")

        agent.reset()

    print("\n  Demo complete.")


def main():
    parser = argparse.ArgumentParser(description="GOT Benchmarking Framework")
    parser.add_argument("--demo", action="store_true", help="Run interactive demo")
    parser.add_argument("--iterations", type=int, default=5, help="Iterations per treatment")
    parser.add_argument("--combinations", action="store_true", help="Include pairwise interaction tests")
    parser.add_argument("--output", type=str, default="outputs", help="Output directory")
    parser.add_argument("--seed", type=int, default=42, help="Random seed")
    parser.add_argument("--scirust", action="store_true", help="Force scirust bridge usage")
    args = parser.parse_args()

    if args.demo:
        asyncio.run(run_interactive_demo())
    else:
        asyncio.run(run_full_benchmark(
            iterations=args.iterations,
            include_combinations=args.combinations,
            output_dir=args.output,
            seed=args.seed,
        ))


if __name__ == "__main__":
    main()
