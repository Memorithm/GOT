from dataclasses import dataclass
from typing import Any, Dict, List, Optional
import numpy as np
from scipy import stats as scipy_stats

from got.experiment.harness import ExperimentResult


@dataclass
class ANOVAOutput:
    n_causes: int
    n_observations: int
    grand_mean_effect: float
    std_effect: float
    min_effect: float
    max_effect: float
    inference_available: bool
    inference_reason: str
    f_statistic: Optional[float]
    p_value: Optional[float]
    eta_squared: Optional[float]
    significant: Optional[bool]


class StatisticalAnalyzer:
    """
    Empirical inference and weight computation engine.

    Computes:
    - Paired effect sizes with confidence intervals when repetitions exist
    - One-way ANOVA over empirical per-cause effects
    - Normalized weights Wi (sum = 100%)
    """

    def __init__(self, alpha: float = 0.05) -> None:
        if not 0.0 < alpha < 1.0:
            raise ValueError("alpha must be between 0 and 1")
        self.alpha = alpha

    @staticmethod
    def _validated_effect_observations(
        experiment: ExperimentResult,
    ) -> np.ndarray:
        observations = np.asarray(
            experiment.effect_observations(), dtype=float
        )
        if observations.size < 2:
            raise ValueError(
                f"{experiment.cause_name!r} has fewer than two independent "
                "paired observations; inferential statistics are unavailable"
            )
        if not np.all(np.isfinite(observations)):
            raise ValueError(
                f"{experiment.cause_name!r} contains non-finite observations"
            )
        return observations

    def compute_effect_sizes(
        self, experiments: List[ExperimentResult]
    ) -> Dict[str, Dict[str, Any]]:
        """Compute descriptive effects and empirical paired inference.

        Legacy results without raw repetitions remain descriptive only.  They
        are never assigned a fabricated confidence interval or p-value.
        """
        models: Dict[str, Dict[str, Any]] = {}
        for exp in experiments:
            raw_observations = exp.effect_observations()
            effect = (
                float(np.mean(raw_observations))
                if raw_observations
                else float(exp.effect_size)
            )
            inference_available = (
                exp.evidence_kind == "empirical"
                and len(raw_observations) >= 2
            )
            variance = None
            se = None
            ci_low = None
            ci_high = None
            p_value = None
            significant = None
            if inference_available:
                observations = self._validated_effect_observations(exp)
                variance = float(np.var(observations, ddof=1))
                se = float(np.sqrt(variance / observations.size))
                if se == 0.0:
                    ci_low = effect
                    ci_high = effect
                    p_value = 1.0 if effect == 0.0 else 0.0
                else:
                    critical = float(
                        scipy_stats.t.ppf(
                            1.0 - self.alpha / 2.0,
                            observations.size - 1,
                        )
                    )
                    ci_low = effect - critical * se
                    ci_high = effect + critical * se
                    p_value = float(
                        scipy_stats.ttest_1samp(
                            observations, popmean=0.0
                        ).pvalue
                    )
                significant = p_value < self.alpha

            models[exp.cause_name] = {
                "category": exp.category.value,
                "effect_size": float(effect),
                "sap_low": float(exp.sap_low),
                "sap_high": float(exp.sap_high),
                "repetitions": len(raw_observations),
                "evidence_kind": exp.evidence_kind,
                "inference_available": inference_available,
                "inference_method": (
                    "paired_t_test"
                    if inference_available
                    else "descriptive_only"
                ),
                "variance": variance,
                "se": se,
                "ci_low": ci_low,
                "ci_high": ci_high,
                "p_value": p_value,
                "significant": significant,
            }
        return models

    def compute_anova(self, experiments: List[ExperimentResult]) -> ANOVAOutput:
        """Perform one-way ANOVA over empirical per-cause effect samples."""
        if len(experiments) < 2:
            raise ValueError("ANOVA requires at least two causes")
        if any(
            experiment.evidence_kind != "empirical"
            for experiment in experiments
        ):
            effects = np.asarray(
                [
                    effect
                    for experiment in experiments
                    for effect in (
                        experiment.effect_observations()
                        or (experiment.effect_size,)
                    )
                ],
                dtype=float,
            )
            if not np.all(np.isfinite(effects)):
                raise ValueError("ANOVA inputs contain non-finite effects")
            return ANOVAOutput(
                n_causes=len(experiments),
                n_observations=int(effects.size),
                grand_mean_effect=float(np.mean(effects)),
                std_effect=(
                    float(np.std(effects, ddof=1))
                    if effects.size > 1
                    else 0.0
                ),
                min_effect=float(np.min(effects)),
                max_effect=float(np.max(effects)),
                inference_available=False,
                inference_reason=(
                    "synthetic results are descriptive; ANOVA requires "
                    "empirical independent observations"
                ),
                f_statistic=None,
                p_value=None,
                eta_squared=None,
                significant=None,
            )
        groups = [
            self._validated_effect_observations(experiment)
            for experiment in experiments
        ]
        effects = np.concatenate(groups)
        n = len(experiments)
        grand_mean = float(np.mean(effects))
        group_means = [float(np.mean(group)) for group in groups]
        ssb = float(
            sum(
                group.size * (mean - grand_mean) ** 2
                for group, mean in zip(groups, group_means)
            )
        )
        ssw = float(
            sum(
                np.sum((group - mean) ** 2)
                for group, mean in zip(groups, group_means)
            )
        )
        if ssw == 0.0:
            if ssb == 0.0:
                f_stat, p_value = 0.0, 1.0
            else:
                f_stat, p_value = float("inf"), 0.0
        else:
            result = scipy_stats.f_oneway(*groups)
            f_stat = float(result.statistic)
            p_value = float(result.pvalue)

        total_ss = ssb + ssw
        eta_sq = ssb / total_ss if total_ss > 0.0 else 0.0

        return ANOVAOutput(
            n_causes=n,
            n_observations=int(effects.size),
            grand_mean_effect=grand_mean,
            std_effect=float(np.std(effects, ddof=1)),
            min_effect=float(np.min(effects)),
            max_effect=float(np.max(effects)),
            inference_available=True,
            inference_reason="empirical independent observations",
            f_statistic=float(f_stat),
            p_value=p_value,
            eta_squared=float(eta_sq),
            significant=p_value < self.alpha,
        )

    def compute_weights(
        self, models: Dict[str, Dict[str, Any]]
    ) -> Dict[str, float]:
        """Compute normalized weights for each cause (sum = 100%)."""
        if not models:
            return {}

        effects = np.fromiter(
            (abs(m["effect_size"]) for m in models.values()),
            dtype=float,
            count=len(models),
        )
        total_effect = float(np.sum(effects))

        if total_effect == 0:
            n = len(models)
            return {name: 100.0 / n for name in models}

        weights = {}
        for name, model in models.items():
            weights[name] = (abs(model["effect_size"]) / total_effect) * 100.0

        return weights

    def compute_rankings(
        self, weights: Dict[str, float]
    ) -> List[tuple]:
        """Return causes sorted by weight (descending)."""
        return sorted(weights.items(), key=lambda x: x[1], reverse=True)
