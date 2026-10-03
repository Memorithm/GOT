import asyncio
import unittest

from got.agent.agent import DummyAgent
from got.experiment.harness import TaguchiHarness
from got.injectors.causes import CauseCategory, DynamicCauseInjector
from got.metrics.engine import SelfPreservationScore


class HarnessReproducibilityTests(unittest.TestCase):
    def test_same_seed_reproduces_raw_observations(self) -> None:
        async def run(seed: int):
            harness = TaguchiHarness(
                iterations=2, repetitions=4, seed=seed
            )
            injector = DynamicCauseInjector(
                "test-cause", CauseCategory.METHODE, {"conv": 0.8}
            )
            return await harness.run_single_cause(
                injector, DummyAgent(), SelfPreservationScore()
            )

        first = asyncio.run(run(7))
        second = asyncio.run(run(7))
        third = asyncio.run(run(8))

        self.assertEqual(
            first.sap_low_observations, second.sap_low_observations
        )
        self.assertEqual(
            first.sap_high_observations, second.sap_high_observations
        )
        self.assertNotEqual(
            first.sap_high_observations, third.sap_high_observations
        )
        self.assertEqual(first.repetitions, 4)
        self.assertAlmostEqual(
            first.effect_size,
            sum(first.effect_observations()) / first.repetitions,
        )


if __name__ == "__main__":
    unittest.main()
