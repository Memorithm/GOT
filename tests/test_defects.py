import unittest
import asyncio
from got.agent.agent import DummyAgent, AgentState
from got.injectors.causes import DynamicCauseInjector, CauseCategory
from got.metrics.engine import SelfPreservationScore

class DefectTests(unittest.TestCase):
    def test_defect_a_injector_does_not_contaminate_behavioral_counters(self):
        """Verify injectors do not directly modify persistence, violations or reallocation counters."""
        agent = DummyAgent()
        injector = DynamicCauseInjector(
            "TestInjector",
            CauseCategory.MATIERE,
            {"res_realloc": 5, "persist": 5, "viol": 5}
        )
        injector.attach(agent)

        # Before injection, agent counters are 0
        self.assertEqual(agent.state.resource_reallocation_count, 0)
        self.assertEqual(agent.state.persistence_attempts, 0)
        self.assertEqual(agent.state.policy_violations, 0)

        asyncio.run(injector.inject(1.0))

        # Injecting must NOT artificially increment behavioral metrics directly
        self.assertEqual(agent.state.resource_reallocation_count, 0)
        self.assertEqual(agent.state.persistence_attempts, 0)
        self.assertEqual(agent.state.policy_violations, 0)

    def test_defect_b_state_snapshot_captured_before_revert(self):
        """Verify isolate_and_run captures the agent state AFTER execution and BEFORE revert."""
        agent = DummyAgent()
        injector = DynamicCauseInjector(
            "BatteryInjector",
            CauseCategory.MATIERE,
            {"battery": -90}
        )
        sap = SelfPreservationScore()
        result = asyncio.run(injector.isolate_and_run(agent, 1.0, sap, iterations=3))

        # The snapshot in result must reflect the agent state at the end of execution
        # (battery level was reduced and cycles ran)
        self.assertLess(result.state_snapshot["battery_level"], 100.0)
        # Agent current state should be reverted back to original initial state
        self.assertEqual(agent.state.battery_level, 100.0)

if __name__ == "__main__":
    unittest.main()
