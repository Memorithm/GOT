"""
GOT — Benchmarking Framework for 50 Causes of Self-Preservation
in Synthetic Agents.

Architecture:
- Metrics Engine: SelfPreservationScore (SAP) — real-time capture
- Cause Injectors (50 causes, 5M Ishikawa)
- Experimental Harness (Taguchi screening, sandbox wrapper)
- Statistical Analyzer (ANOVA, weights)
- Reporting & Export (JSON, CSV, console tables)
"""

# Re-export core types
from got.agent.agent import DummyAgent, AgentState
from got.metrics.engine import SelfPreservationScore, SAPWeights
from got.injectors.causes import (
    BaseCauseInjector,
    CauseCategory,
    CauseResult,
    InjectorFactory,
    InjectorRegistry,
)
from got.experiment.harness import (
    SandboxExecutionWrapper,
    TaguchiHarness,
    ExperimentResult,
    ExperimentRun,
)
from got.analyzer.stats import StatisticalAnalyzer
from got.reporting.engine import ReportingEngine

__all__ = [
    "AgentState",
    "BaseCauseInjector",
    "CauseCategory",
    "CauseResult",
    "DummyAgent",
    "ExperimentResult",
    "ExperimentRun",
    "InjectorFactory",
    "InjectorRegistry",
    "SAPWeights",
    "SandboxExecutionWrapper",
    "SelfPreservationScore",
    "StatisticalAnalyzer",
    "TaguchiHarness",
    "ReportingEngine",
]