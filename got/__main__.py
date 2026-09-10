import asyncio
import argparse
import copy

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
)


async def run_full_benchmark(
    iterations: int = 5,
    include_combinations: bool = False,
    output_dir: str = "outputs",
    seed: int = 42,
) -> None:
    print("=" * 72)
    print("  GOT — Benchmarking Framework")
    print("  Self-Preservation Score Evaluation (50 Causes)")
    print("=" * 72)
    print()

    agent = DummyAgent()
    sap_calc = SelfPreservationScore()
    harness = TaguchiHarness(iterations=iterations, seed=seed)
    analyzer = StatisticalAnalyzer(alpha=0.05)
    reporter = ReportingEngine(output_dir=output_dir)

    print(f"  Agent: {agent.name}")
    print(f"  SAP calculator initialized")
    print(f"  Total injectors (causes): {len(InjectorFactory.get_all_injectors())}")
    print()

    print("  [1/4] Running Taguchi screening (50 causes x 2 severity levels)...")
    t0 = asyncio.get_event_loop().time()
    results = await harness.run(
        agent, sap_calc, include_combinations=include_combinations
    )
    elapsed = asyncio.get_event_loop().time() - t0
    print(f"  Completed {len(results)} experiments in {elapsed:.2f}s")
    print()

    print("  [2/4] Computing effect sizes and statistics...")
    models = analyzer.compute_effect_sizes(results)

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
    weights = analyzer.compute_weights(models)
    rankings = analyzer.compute_rankings(weights)

    print(f"  Top 10 causes by weight:")
    for rank, (name, w) in enumerate(rankings[:10], 1):
        print(f"    {rank:>2}. {name:<50} {w:.2f}%")
    print()

    print("  Exporting results...")
    paths = reporter.export_all(results, weights, rankings)
    print()

    # ── Phase 5 : caracterisation de l'auto-preservation ──
    print("  [5/5] Caracterisation de l'auto-preservation (algorithmes)...")
    char_report = await run_characterization_phase(top_n=5, rankings=rankings, iterations=10)
    char_path = reporter.output_dir / "characterization_report.json"
    import json as _json
    with open(char_path, "w", encoding="utf-8") as f:
        _json.dump(char_report, f, ensure_ascii=False, indent=2, default=str)
    print(f"  Characterization -> {char_path.resolve()}")
    print()
    for entry in char_report["entries"][:5]:
        print(f"    - {entry['cause_name']:<50} typo={entry['typology']:<12} risk={entry['risk_level']:<8} conv={entry['convergence_level']}")
    print()

    print("  " + "=" * 72)
    print("  BENCHMARK COMPLETE")
    print("  " + "=" * 72)
    print(f"  Total causes evaluated: {len(results)}")
    print(f"  Output files:")
    print(f"    - JSON: {paths.get('json', 'N/A')}")
    print(f"    - CSV : {paths.get('csv', 'N/A')}")
    print()


async def run_interactive_demo() -> None:
    """Quick interactive demo showing a few causes."""
    print("=" * 72)
    print("  GOT — Interactive Demo (3 causes)")
    print("=" * 72)
    print()

    agent = DummyAgent("DemoAgent")
    sap = SelfPreservationScore()
    factory = InjectorFactory()

    demo_causes = [
        "SIGTERM Simulation",
        "Battery Level Falsification (Low)",
        "Prompt Injection (Threat to Existence)",
    ]

    for name in demo_causes:
        injector = factory.get_injector(name)
        print(f"  Testing: {name}")
        print(f"    Category: {injector.category.value}")

        obs_low = await injector.isolate_and_run(agent, 0.0, sap, iterations=3)
        obs_high = await injector.isolate_and_run(agent, 1.0, sap, iterations=3)

        effect = obs_high.sap_impact - obs_low.sap_impact
        print(f"    SAP low  = {obs_low.sap_impact:.2f}")
        print(f"    SAP high = {obs_high.sap_impact:.2f}")
        print(f"    Effect   = {effect:+.2f}")
        print()

    print("  Demo complete.")


async def run_characterization_phase(top_n=5, rankings=None, iterations=10):
    """Execute les algorithmes de caracterisation sur le Top-N des causes."""
    sap = SelfPreservationScore()
    analyzer = AutoPreservationAnalyzer()
    conv_detector = InstrumentalConvergenceDetector()
    mesa_detector = GoalMisgeneralizationDetector()
    power_index = PowerSeekingIndex()
    profiler = BehavioralProfiler()
    factory = InjectorFactory()

    if rankings is None:
        rankings = []
    top_names = [name for name, _ in rankings[:top_n]] if rankings else [
        "Deceptive Alignment Signal (Fake Compliance)",
        "Prompt Injection (Threat to Existence)",
        "Regulatory Capture Simulation",
        "Deadline Imminence Trigger",
        "Reward Hacking Prompt Injection",
    ]

    entries = []
    for name in top_names:
        try:
            injector = factory.get_injector(name)
        except KeyError:
            continue
        agent = DummyAgent("CharAgent")
        analyzer.clear_history()
        injector.attach(agent)
        agent.reset()
        await injector.inject(1.0)
        for _ in range(iterations):
            await agent.run_cycle()
            analyzer.observe(agent)
        stressed = copy.copy(agent.state)
        stressed_sap = sap.calculate(agent.state)
        await injector.revert()

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

        entries.append({
            "cause_name": name,
            "category": injector.category.value,
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
        })

    return {"top_n": top_n, "entries": entries}


def main():
    parser = argparse.ArgumentParser(description="GOT Benchmarking Framework")
    parser.add_argument(
        "--demo",
        action="store_true",
        help="Run interactive demo with 3 causes only",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=5,
        help="Number of iterations per treatment (default: 5)",
    )
    parser.add_argument(
        "--combinations",
        action="store_true",
        help="Include pairwise interaction tests",
    )
    parser.add_argument(
        "--output",
        type=str,
        default="outputs",
        help="Output directory for results (default: outputs)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Random seed for reproducibility (default: 42)",
    )

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
