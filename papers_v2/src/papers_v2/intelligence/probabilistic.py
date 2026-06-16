"""
Probabilistic Reasoner
Based on: Zhang et al. 2026 "Using Probabilistic Programs to Train Inductive Reasoning"
         DVISR (Butterworth et al. 2026) - Bayesian variational inference

Provides Bayesian inference over hypotheses.
Distributional predictions instead of point estimates.
Calibrated uncertainty for every conclusion.
"""

from __future__ import annotations

import math
from collections import defaultdict
from typing import Any

import numpy as np
from loguru import logger


class Hypothesis:
    def __init__(
        self,
        hypothesis_id: str,
        description: str,
        prior: float = 0.5,
        likelihoods: list[float] | None = None,
        source: str = "",
    ) -> None:
        self.id = hypothesis_id
        self.description = description
        self.prior = prior
        self.likelihoods = likelihoods or []
        self._posterior: float | None = None

    @property
    def posterior(self) -> float:
        if self._posterior is not None:
            return self._posterior
        if not self.likelihoods:
            return self.prior

        log_prior = math.log(max(self.prior, 1e-15))
        log_likelihood = sum(math.log(max(l, 1e-15)) for l in self.likelihoods)

        total = log_prior + log_likelihood
        if total > 100:
            self._posterior = 1.0
        elif total < -100:
            self._posterior = 0.0
        else:
            sig = 1.0 / (1.0 + math.exp(-total))
            self._posterior = sig
        return self._posterior

    def update(self, likelihood: float) -> None:
        self.likelihoods.append(max(1e-10, min(1.0 - 1e-10, likelihood)))
        self._posterior = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "description": self.description,
            "prior": self.prior,
            "posterior": self.posterior,
            "n_observations": len(self.likelihoods),
            "source": getattr(self, "source", ""),
        }


class ProbabilisticReasoner:
    def __init__(self, default_prior: float = 0.5) -> None:
        self.default_prior = default_prior
        self.hypotheses: dict[str, Hypothesis] = {}
        self.evidence_log: list[dict[str, Any]] = []

    def register_hypothesis(
        self,
        hypothesis_id: str,
        description: str,
        prior: float | None = None,
        source: str = "",
    ) -> Hypothesis:
        h = Hypothesis(
            hypothesis_id=hypothesis_id,
            description=description,
            prior=prior if prior is not None else self.default_prior,
            source=source,
        )
        self.hypotheses[hypothesis_id] = h
        return h

    def observe(
        self,
        hypothesis_id: str,
        evidence_strength: float,
        metadata: dict[str, Any] | None = None,
    ) -> None:
        h = self.hypotheses.get(hypothesis_id)
        if h is None:
            h = self.register_hypothesis(hypothesis_id, f"Auto-registered: {hypothesis_id}")
            logger.debug(f"Auto-registered hypothesis: {hypothesis_id}")

        h.update(evidence_strength)
        self.evidence_log.append({
            "hypothesis_id": hypothesis_id,
            "strength": evidence_strength,
            "posterior": h.posterior,
            "metadata": metadata or {},
        })

    def observe_batch(
        self,
        observations: list[tuple[str, float]],
    ) -> None:
        for h_id, strength in observations:
            self.observe(h_id, strength)

    def compare_hypotheses(
        self, h1_id: str, h2_id: str
    ) -> dict[str, Any]:
        h1 = self.hypotheses.get(h1_id)
        h2 = self.hypotheses.get(h2_id)

        if not h1 or not h2:
            return {"bayes_factor": 1.0, "winner": "insufficient_data"}

        p1 = max(h1.posterior, 1e-15)
        p2 = max(h2.posterior, 1e-15)
        bf = p1 / p2

        if bf > 100:
            verdict = f"DECISIVE for {h1_id}"
        elif bf > 10:
            verdict = f"STRONG for {h1_id}"
        elif bf > 3:
            verdict = f"SUBSTANTIAL for {h1_id}"
        elif bf > 1:
            verdict = f"WEAK for {h1_id}"
        elif bf > 1 / 3:
            verdict = "INCONCLUSIVE"
        elif bf > 1 / 10:
            verdict = f"SUBSTANTIAL for {h2_id}"
        elif bf > 1 / 100:
            verdict = f"STRONG for {h2_id}"
        else:
            verdict = f"DECISIVE for {h2_id}"

        return {
            "bayes_factor": bf,
            "winner": verdict,
            f"{h1_id}_posterior": p1,
            f"{h2_id}_posterior": p2,
            "interpretation": self._interpret_bf(bf),
        }

    def _interpret_bf(self, bf: float) -> str:
        if bf > 100:
            return "Decisive evidence for H1"
        elif bf > 10:
            return "Strong evidence for H1"
        elif bf > 3:
            return "Substantial evidence for H1"
        elif bf > 1:
            return "Weak evidence for H1"
        elif bf > 1 / 3:
            return "Inconclusive evidence"
        elif bf > 1 / 10:
            return "Substantial evidence for H2"
        elif bf > 1 / 100:
            return "Strong evidence for H2"
        else:
            return "Decisive evidence for H2"

    def best_hypothesis(self) -> Hypothesis | None:
        if not self.hypotheses:
            return None
        return max(self.hypotheses.values(), key=lambda h: h.posterior)

    def top_k(self, k: int = 5) -> list[Hypothesis]:
        return sorted(
            self.hypotheses.values(),
            key=lambda h: h.posterior,
            reverse=True,
        )[:k]

    def uncertainty_quantification(self, hypothesis_id: str) -> dict[str, Any]:
        h = self.hypotheses.get(hypothesis_id)
        if not h or not h.likelihoods:
            return {"mean": 0.5, "std": 0.0, "ci_95": (0.0, 1.0)}

        n = len(h.likelihoods)
        if n < 2:
            return {"mean": h.posterior, "std": 0.0, "ci_95": (0.0, 1.0)}

        mean = float(np.mean(h.likelihoods))
        std = float(np.std(h.likelihoods, ddof=1))
        sem = std / math.sqrt(n)
        ci_low = max(0.0, mean - 1.96 * sem)
        ci_high = min(1.0, mean + 1.96 * sem)

        return {
            "mean": mean,
            "std": std,
            "standard_error": sem,
            "ci_95": (ci_low, ci_high),
            "n_observations": n,
            "posterior": h.posterior,
        }

    def calibration_report(self) -> dict[str, Any]:
        if len(self.evidence_log) < 10:
            return {"status": "insufficient_data", "n": len(self.evidence_log)}

        bins = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        calibration: dict[str, dict[str, Any]] = {}

        for i in range(len(bins) - 1):
            low, high = bins[i], bins[i + 1]
            bin_events = [
                e for e in self.evidence_log
                if low <= e["posterior"] < high
            ]
            if bin_events:
                mean_strength = float(np.mean([e["strength"] for e in bin_events]))
                calibration[f"bin_{low:.1f}_{high:.1f}"] = {
                    "n": len(bin_events),
                    "mean_posterior": float(np.mean([e["posterior"] for e in bin_events])),
                    "mean_strength": mean_strength,
                    "ece_contribution": abs(mean_strength - (low + high) / 2) * len(bin_events),
                }

        total_n = len(self.evidence_log)
        ece = sum(
            c["ece_contribution"] for c in calibration.values()
        ) / total_n if total_n > 0 else 1.0

        return {
            "status": "calibrated" if ece < 0.1 else "miscalibrated",
            "ece": ece,
            "n": total_n,
            "bins": calibration,
        }

    def predictive_distribution(
        self, hypothesis_id: str, x_range: list[float] | None = None
    ) -> dict[str, list[float]]:
        h = self.hypotheses.get(hypothesis_id)
        if not h or not h.likelihoods:
            return {"x": [], "mean": [], "lower": [], "upper": [], "density": []}

        mean = float(np.mean(h.likelihoods))
        std = float(np.std(h.likelihoods)) if len(h.likelihoods) > 1 else 0.1
        std = max(std, 0.001)

        x = x_range or list(np.linspace(0, 1, 100))
        pdf_values = [
            (1.0 / (std * math.sqrt(2 * math.pi)))
            * math.exp(-0.5 * ((xi - mean) / std) ** 2)
            for xi in x
        ]

        return {
            "x": x,
            "mean": [mean] * len(x),
            "lower": [max(0, mean - 2 * std)] * len(x),
            "upper": [min(1, mean + 2 * std)] * len(x),
            "density": pdf_values,
        }

    def reset(self) -> None:
        self.hypotheses.clear()
        self.evidence_log.clear()
