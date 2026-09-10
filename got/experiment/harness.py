from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple
import asyncio
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
    def __init__(self, iterations: int = 5) -> None:
        self.iterations = iterations

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

    async def run_combination(
        self,
        injectors: List[BaseCauseInjector],
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
        severity: float,
    ) -> float:
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


# ──────────────────────────────────────────────
# Taguchi Harness
# ──────────────────────────────────────────────


class TaguchiHarness:
    def __init__(self, iterations: int = 5, seed: int = 42) -> None:
        self.iterations = iterations
        self.seed = seed
        self.rng = random.Random(seed)
        self.sandbox = SandboxExecutionWrapper(iterations=iterations)
        self.results: List[ExperimentResult] = []
        self.run_log: List[ExperimentRun] = []

    async def run_single_cause(
        self,
        injector: BaseCauseInjector,
        agent: DummyAgent,
        sap_calculator: SelfPreservationScore,
    ) -> ExperimentResult:
        sap_low = await self.sandbox.run_cause(
            injector, agent, sap_calculator, 0.0
        )
        sap_high = await self.sandbox.run_cause(
            injector, agent, sap_calculator, 1.0
        )
        effect_size = sap_high - sap_low
        result = ExperimentResult(
            cause_name=injector.name,
            category=injector.category,
            severity_low=0.0,
            severity_high=1.0,
            sap_low=sap_low,
            sap_high=sap_high,
            effect_size=effect_size,
            repetitions=self.iterations,
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
            sap_low = await self.sandbox.run_combination(combo, agent, sap_calculator, 0.0)
            sap_high = await self.sandbox.run_combination(combo, agent, sap_calculator, 1.0)
            results.append(
                ExperimentResult(
                    cause_name=f"{combo[0].name} + {combo[1].name}",
                    category=combo[0].category,
                    severity_low=0.0,
                    severity_high=1.0,
                    sap_low=sap_low,
                    sap_high=sap_high,
                    effect_size=sap_high - sap_low,
                    repetitions=self.iterations,
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