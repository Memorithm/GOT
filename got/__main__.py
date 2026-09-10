import asyncio
import argparse

from got import (
    DummyAgent,
    InjectorFactory,
    ReportingEngine,
    SelfPreservationScore,
    StatisticalAnalyzer,
    TaguchiHarness,
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
