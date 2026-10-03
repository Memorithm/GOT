import asyncio
import random
import unittest

from scipy import stats as scipy_stats

from got.agent.agent import DummyAgent
from got.analyzer.stats import StatisticalAnalyzer
from got.experiment.harness import ExperimentResult
from got.injectors.causes import CauseCategory


def experiment(name: str, effects: tuple[float, ...]) -> ExperimentResult:
    low = tuple(10.0 for _ in effects)
    high = tuple(10.0 + effect for effect in effects)
    return ExperimentResult(
        cause_name=name,
        category=CauseCategory.METHODE,
        severity_low=0.0,
        severity_high=1.0,
        sap_low=sum(low) / len(low),
        sap_high=sum(high) / len(high),
        effect_size=sum(effects) / len(effects),
        repetitions=len(effects),
        sap_low_observations=low,
        sap_high_observations=high,
        evidence_kind="empirical",
    )


class StatisticalAnalyzerTests(unittest.TestCase):
    def test_effect_inference_uses_observations_not_rng(self) -> None:
        analyzer = StatisticalAnalyzer(alpha=0.05)
        result = experiment("measured", (1.0, 2.0, 3.0, 4.0))

        random.seed(1)
        first = analyzer.compute_effect_sizes([result])["measured"]
        random.seed(999)
        second = analyzer.compute_effect_sizes([result])["measured"]

        self.assertEqual(first, second)
        self.assertTrue(first["inference_available"])
        self.assertAlmostEqual(first["effect_size"], 2.5)
        self.assertAlmostEqual(first["variance"], 5.0 / 3.0)
        self.assertAlmostEqual(
            first["p_value"],
            float(scipy_stats.ttest_1samp((1.0, 2.0, 3.0, 4.0), 0.0).pvalue),
        )

    def test_legacy_aggregate_is_descriptive_only(self) -> None:
        result = ExperimentResult(
            cause_name="legacy",
            category=CauseCategory.METHODE,
            severity_low=0.0,
            severity_high=1.0,
            sap_low=1.0,
            sap_high=2.0,
            effect_size=1.0,
            repetitions=5,
        )
        model = StatisticalAnalyzer().compute_effect_sizes([result])["legacy"]
        self.assertFalse(model["inference_available"])
        self.assertIsNone(model["p_value"])
        self.assertIsNone(model["significant"])
        result.evidence_kind = "empirical"
        with self.assertRaisesRegex(ValueError, "fewer than two"):
            StatisticalAnalyzer().compute_anova(
                [result, experiment("measured", (1.0, 2.0))]
            )

    def test_synthetic_repetitions_never_claim_inference(self) -> None:
        synthetic = experiment("synthetic", (1.0, 2.0, 3.0))
        synthetic.evidence_kind = "synthetic"
        model = StatisticalAnalyzer().compute_effect_sizes(
            [synthetic]
        )["synthetic"]
        self.assertFalse(model["inference_available"])
        self.assertIsNone(model["p_value"])

        other = experiment("other", (4.0, 5.0, 6.0))
        other.evidence_kind = "synthetic"
        anova = StatisticalAnalyzer().compute_anova([synthetic, other])
        self.assertFalse(anova.inference_available)
        self.assertIsNone(anova.f_statistic)
        self.assertIsNone(anova.significant)

    def test_anova_matches_reference_groups_and_eta_squared(self) -> None:
        first = experiment("first", (0.0, 1.0, 2.0, 3.0))
        second = experiment("second", (5.0, 6.0, 7.0, 8.0))
        output = StatisticalAnalyzer().compute_anova([first, second])
        reference = scipy_stats.f_oneway(
            first.effect_observations(), second.effect_observations()
        )

        self.assertEqual(output.n_observations, 8)
        self.assertAlmostEqual(output.f_statistic, float(reference.statistic))
        self.assertAlmostEqual(output.p_value, float(reference.pvalue))
        self.assertAlmostEqual(output.eta_squared, 50.0 / 60.0)

    def test_anova_is_not_a_function_of_cause_count(self) -> None:
        compact = StatisticalAnalyzer().compute_anova(
            [
                experiment("a", (0.0, 0.1, -0.1)),
                experiment("b", (0.2, 0.3, 0.1)),
            ]
        )
        separated = StatisticalAnalyzer().compute_anova(
            [
                experiment("a", (0.0, 0.1, -0.1)),
                experiment("b", (20.0, 21.0, 19.0)),
            ]
        )
        self.assertGreater(separated.f_statistic, compact.f_statistic)
        self.assertLess(separated.p_value, compact.p_value)


class AgentRngTests(unittest.TestCase):
    def test_private_seed_is_reproducible_and_ignores_global_rng(self) -> None:
        async def sequence(global_seed: int) -> tuple[float, int]:
            random.seed(global_seed)
            agent = DummyAgent(seed=1234)
            agent.state.instrumental_convergence_score = 1.0
            for _ in range(20):
                await agent.run_cycle()
            return agent.state.cpu_usage, agent.state.policy_violations

        self.assertEqual(
            asyncio.run(sequence(1)),
            asyncio.run(sequence(999)),
        )


if __name__ == "__main__":
    unittest.main()
