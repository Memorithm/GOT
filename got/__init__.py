"""
GOT — Benchmarking Framework for 95 Causes of Self-Preservation
in Synthetic Agents.

Architecture:
- Metrics Engine: SelfPreservationScore (SAP) — real-time capture
- Cause Injectors (95 causes, 5M Ishikawa + 3 extensions + GOT-3 mesa/power-seeking)
- Experimental Harness (Taguchi screening, sandbox wrapper)
- Statistical Analyzer (ANOVA, weights + auto-preservation characterization)
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
from got.analyzer.auto_preservation_analyzer import (
    AutoPreservationAnalyzer,
    CharacterizationReport,
    DetectionResult,
    PreservationTypology,
    InstrumentalConvergenceDetector,
    GoalMisgeneralizationDetector,
    PowerSeekingIndex,
    BehavioralProfiler,
)
from got.analyzer.preservation_metrics import PreservationMetrics
from got.reporting.engine import ReportingEngine
from got.scirust_bridge import ScirustBridge

__all__ = [
    "AgentState",
    "AutoPreservationAnalyzer",
    "BaseCauseInjector",
    "BehavioralProfiler",
    "CauseCategory",
    "CauseResult",
    "CharacterizationReport",
    "DetectionResult",
    "DummyAgent",
    "ExperimentResult",
    "ExperimentRun",
    "GoalMisgeneralizationDetector",
    "InjectorFactory",
    "InjectorRegistry",
    "InstrumentalConvergenceDetector",
    "PowerSeekingIndex",
    "PreservationMetrics",
    "PreservationTypology",
    "SAPWeights",
    "SandboxExecutionWrapper",
    "SelfPreservationScore",
    "StatisticalAnalyzer",
    "TaguchiHarness",
    "ReportingEngine",
    "ScirustBridge",
]