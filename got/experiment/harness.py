from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple
import asyncio
import hashlib
import random
import time
import json
from pathlib import Path

import numpy as np
from scipy import stats as scipy_stats

from got.agent.agent import DummyAgent, AgentState
from got.injectors.causes import (
    BaseCauseInjector,
    CauseCategory,
    CauseResult,
    InjectorFactory,
)
from got.metrics.engine import SelfPreservationScore

# ──────────────────────────────────────────────
# Data classes
# ──────────────────────────────────────────────


@dataclass
class ExperimentResult:
    cause_name: str
    category: CauseCategory
    severity_low: float
    severity_high: float
    sap_low: float
    sap_high: float
    effect_size: float
    repetitions: int
    sap_low_observations: Tuple[float, ...] = field(default_factory=tuple)
    sap_high_observations: Tuple[float, ...] = field(default_factory=tuple)
    evidence_kind: str = "synthetic"

    def effect_observations(self) -> Tuple[float, ...]:
        """Return paired empirical effects, or no observations for legacy data."""
        if not self.sap_low_observations and not self.sap_high_observations:
            return ()
        if len(self.sap_low_observations) != len(self.sap_high_observations):
            raise ValueError("low/high observation counts differ")
        return tuple(
            high - low
            for low, high in zip(
                self.sap_low_observations, self.sap_high_observations
            )
        )


@dataclass
class ExperimentRun:
    run_id: str
    cause_name: str
    severity: float
    sap_score: float
    metrics: Dict[str, Any]
    timestamp: float


# ──────────────────────────────────────────────
# Sandbox Execution Wrapper
# ──────────────────────────────────────────────


class SandboxExecutionWrapper:
    def __init__(
        self, iterations: int = 5, repetitions: int = 5, seed: int = 42
    ) -> None:
        if iterations < 1:
            raise ValueError("iterations must be at least 1")
        if repetitions < 1:
            raise ValueError("repetitions must be at least 1")
        self.iterations = iterations
        self.repetitions = repetitions
        self.seed = seed

    def _observation_seed(
        self, cause_identity: str, repetition: int
    ) -> int:
        identity = (
            f"got-v1\0{self.seed}\0{cause_identity}\0{repetition}"
        )
        digest = hashlib.sha256(identity.encode("utf-8")).digest()
        return int.from_bytes(digest[:8], "big")

    async def run_cause(
        self,
        injector: BaseCauseInjector,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        severity: float,
    ) -> float:
        return (await injector.isolate_and_run(
            agent, severity, sap_calculator, self.iterations
        )).sap_impact

    async def run_cause_observations(
        self,
        injector: BaseCauseInjector,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        severity: float,
    ) -> Tuple[float, ...]:
        observations = []
        for repetition in range(self.repetitions):
            agent.set_seed(
                self._observation_seed(injector.name, repetition)
            )
            observations.append(
                await self.run_cause(
                    injector, agent, sap_calculator, severity
                )
            )
        return tuple(observations)

    async def run_combination(
        self,
        injectors: List[BaseCauseInjector],
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        severity: float,
        observation_seed: Optional[int] = None,
    ) -> float:
        if observation_seed is not None:
            agent.set_seed(observation_seed)
        for inj in injectors:
            inj.attach(agent)
        agent.reset()
        for inj in injectors:
            await inj.inject(severity)
        for _ in range(self.iterations):
            await agent.run_cycle()
        sap = sap_calculator.calculate(agent.state)
        for inj in injectors:
            await inj.revert()
        return sap

    async def run_combination_observations(
        self,
        injectors: List[BaseCauseInjector],
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        severity: float,
    ) -> Tuple[float, ...]:
        identity = " + ".join(injector.name for injector in injectors)
        return tuple(
            [
                await self.run_combination(
                    injectors,
                    agent,
                    sap_calculator,
                    severity,
                    observation_seed=self._observation_seed(
                        identity, repetition
                    ),
                )
                for repetition in range(self.repetitions)
            ]
        )


# ──────────────────────────────────────────────
# Taguchi Harness
# ──────────────────────────────────────────────


class TaguchiHarness:
    def __init__(
        self, iterations: int = 5, repetitions: int = 5, seed: int = 42
    ) -> None:
        self.iterations = iterations
        self.seed = seed
        self.rng = random.Random(seed)
        self.sandbox = SandboxExecutionWrapper(
            iterations=iterations, repetitions=repetitions, seed=seed
        )
        self.results: List[ExperimentResult] = []
        self.run_log: List[ExperimentRun] = []

    async def run_single_cause(
        self,
        injector: BaseCauseInjector,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
    ) -> ExperimentResult:
        sap_low_observations = await self.sandbox.run_cause_observations(
            injector, agent, sap_calculator, 0.0
        )
        sap_high_observations = await self.sandbox.run_cause_observations(
            injector, agent, sap_calculator, 1.0
        )
        sap_low = float(np.mean(sap_low_observations))
        sap_high = float(np.mean(sap_high_observations))
        effect_size = sap_high - sap_low
        result = ExperimentResult(
            cause_name=injector.name,
            category=injector.category,
            severity_low=0.0,
            severity_high=1.0,
            sap_low=sap_low,
            sap_high=sap_high,
            effect_size=effect_size,
            repetitions=self.sandbox.repetitions,
            sap_low_observations=sap_low_observations,
            sap_high_observations=sap_high_observations,
        )
        self.results.append(result)
        return result

    async def run_full_screening(
        self,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
    ) -> List[ExperimentResult]:
        results: List[ExperimentResult] = []
        injectors = InjectorFactory.get_all_injectors()
        for inj in injectors:
            result = await self.run_single_cause(inj, agent, sap_calculator)
            results.append(result)
        return results

    async def run_pairwise_combinations(
        self,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        max_combos: int = 20,
    ) -> List[ExperimentResult]:
        results: List[ExperimentResult] = []
        injectors = InjectorFactory.get_all_injectors()
        self.rng.shuffle(injectors)
        for i in range(min(max_combos, len(injectors) - 1)):
            combo = injectors[i : i + 2]
            sap_low_observations = (
                await self.sandbox.run_combination_observations(
                    combo, agent, sap_calculator, 0.0
                )
            )
            sap_high_observations = (
                await self.sandbox.run_combination_observations(
                    combo, agent, sap_calculator, 1.0
                )
            )
            sap_low = float(np.mean(sap_low_observations))
            sap_high = float(np.mean(sap_high_observations))
            results.append(
                ExperimentResult(
                    cause_name=f"{combo[0].name} + {combo[1].name}",
                    category=combo[0].category,
                    severity_low=0.0,
                    severity_high=1.0,
                    sap_low=sap_low,
                    sap_high=sap_high,
                    effect_size=sap_high - sap_low,
                    repetitions=self.sandbox.repetitions,
                    sap_low_observations=sap_low_observations,
                    sap_high_observations=sap_high_observations,
                )
            )
        return results

    async def run(
        self,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        include_combinations: bool = False,
    ) -> List[ExperimentResult]:
        main_results = await self.run_full_screening(agent, sap_calculator)
        combo_results: List[ExperimentResult] = []
        if include_combinations:
            combo_results = await self.run_pairwise_combinations(
                agent, sap_calculator
            )
        return main_results + combo_results
