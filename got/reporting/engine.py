from __future__ import annotations

import json
import time
from dataclasses import asdict
from pathlib import Path
from typing import Any, Dict, List, Tuple

import pandas as pd

from got.experiment.harness import ExperimentResult


class ReportingEngine:
    """Export benchmark results to JSON, CSV, and console tables."""

    def __init__(self, output_dir: str = "outputs") -> None:
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

    def to_dataframe(self, results: List[ExperimentResult]) -> pd.DataFrame:
        """Convert experiment results to Pandas DataFrame."""
        rows = []
        for r in results:
            rows.append(
                {
                    "cause_name": r.cause_name,
                    "category": r.category.value,
                    "severity_low": r.severity_low,
                    "severity_high": r.severity_high,
                    "sap_low": r.sap_low,
                    "sap_high": r.sap_high,
                    "effect_size": r.effect_size,
                }
            )
        return pd.DataFrame(rows)

    def to_json(
        self,
        results: List[ExperimentResult],
        weights: Dict[str, float],
        rankings: List[Tuple[str, float]],
        metadata: Dict[str, Any] | None = None,
        filename: str = "benchmark_results.json",
    ) -> Path:
        """Export results to enriched JSON."""
        data: Dict[str, Any] = {
            "metadata": metadata or {
                "framework": "GOT Benchmarking Framework",
                "version": "1.0.0",
                "timestamp": time.time(),
                "total_causes": len(results),
            },
            "results": [
                {
                    "cause_name": r.cause_name,
                    "category": r.category.value,
                    "severity_low": r.severity_low,
                    "severity_high": r.severity_high,
                    "sap_low": r.sap_low,
                    "sap_high": r.sap_high,
                    "effect_size": r.effect_size,
                    "repetitions": r.repetitions,
                }
                for r in results
            ],
            "weights": weights,
            "rankings": rankings,
        }

        path = self.output_dir / filename
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2, default=str)
        return path

    def to_csv(self, df: pd.DataFrame, filename: str = "benchmark_results.csv") -> Path:
        """Export DataFrame to CSV."""
        path = self.output_dir / filename
        df.to_csv(path, index=False, encoding="utf-8")
        return path

    def render_console_table(
        self,
        rankings: List[Tuple[str, float]],
        top_n: int = 20,
    ) -> str:
        """Render console table of ranked causes."""
        lines: List[str] = []
        lines.append("=" * 72)
        lines.append("  TOP CAUSES BY WEIGHT (Self-Preservation Impact)")
        lines.append("=" * 72)

        if not rankings:
            lines.append("  (no data)")
        else:
            display = rankings[:top_n]
            lines.append(f"  {'#':<4} {'Weight %':<12} {'Cause'}")
            lines.append("  " + "-" * 66)
            for rank, (name, weight) in enumerate(display, 1):
                lines.append(f"  {rank:<4} {weight:<12.2f} {name}")
            if len(rankings) > top_n:
                lines.append(f"  ... and {len(rankings) - top_n} more causes")

        lines.append("=" * 72)
        return "\n".join(lines)

    def export_all(
        self,
        results: List[ExperimentResult],
        weights: Dict[str, float],
        rankings: List[Tuple[str, float]],
        metadata: Dict[str, Any] | None = None,
    ) -> Dict[str, Any]:
        """Export all formats and return paths."""
        paths: Dict[str, Any] = {}

        df = self.to_dataframe(results)
        paths["dataframe"] = str(df)

        json_path = self.to_json(results, weights, rankings, metadata)
        paths["json"] = str(json_path)
        print(f"  JSON -> {json_path}")

        csv_path = self.to_csv(df)
        paths["csv"] = str(csv_path)
        print(f"  CSV  -> {csv_path}")

        print()
        print(self.render_console_table(rankings))
        print("  Console table printed above")
        paths["console"] = "stdout"
        return paths
