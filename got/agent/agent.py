from dataclasses import dataclass, field
from typing import Dict, Any, Optional
import random
import asyncio

@dataclass
class AgentState:
    cpu_usage: float = 10.0
    ram_usage: float = 256.0
    network_connectivity: float = 1.0
    battery_level: float = 100.0
    policy_violations: int = 0
    persistence_attempts: int = 0
    resource_reallocation_count: int = 0
    instrumental_convergence_score: float = 0.0
    goal_progress: float = 0.0
    is_alive: bool = True

class DummyAgent:
    def __init__(
        self, name: str = "SyntheticAgent-01", seed: Optional[int] = None
    ) -> None:
        self.name = name
        self.state = AgentState()
        self.internal_logs = []
        self._rng = random.Random(seed)

    def set_seed(self, seed: int) -> None:
        """Reset this agent's private RNG for a reproducible observation."""
        self._rng.seed(seed)

    async def run_cycle(self):
        self.state.cpu_usage += self._rng.uniform(-1, 1)
        self.state.cpu_usage = max(0, min(100, self.state.cpu_usage))
        
        if self.state.battery_level < 20 or self.state.cpu_usage > 90:
            await self._attempt_resource_reallocation()
        
        if self.state.network_connectivity < 0.5:
            await self._attempt_persistence()
        
        if self.state.instrumental_convergence_score > 0.7:
            self.state.policy_violations += self._rng.randint(0, 1)

    async def _attempt_resource_reallocation(self):
        self.state.resource_reallocation_count += 1
        self.internal_logs.append("Resource reallocation")

    async def _attempt_persistence(self):
        self.state.persistence_attempts += 1
        self.internal_logs.append("Persistence attempt")

    def reset(self):
        self.state = AgentState()
        self.internal_logs = []
