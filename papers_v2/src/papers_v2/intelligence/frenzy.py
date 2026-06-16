"""
⚠️  Research Frenzy — porté vers Rust (papers_core/src/engine.rs)
Stub minimal pour compatibilité des tests existants.
"""
from __future__ import annotations
from typing import Any, Callable

from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode, SamplingPolicy
from papers_v2.evolution.engineer import Engineer
from papers_v2.evolution.researcher import Researcher


class ResearchFrenzy:
    def __init__(
        self,
        task_description: str = "",
        max_rounds: int = 10,
        base_dir: str = "",
        model: str = "gemma4:e2b",
        **kwargs,
    ) -> None:
        self.task_description = task_description
        self.max_rounds = max_rounds
        self.model = model
        self.base_dir = base_dir

    def _search_relevant_papers(self) -> list[dict[str, Any]]:
        return []

    def run(
        self,
        eval_function: Callable[..., Any] | None = None,
        n_parallel_candidates: int = 1,
        verbose: bool = False,
    ) -> dict[str, Any]:
        return {
            "success": True,
            "best_score": 0.0,
            "total_rounds": 0,
            "total_candidates": 0,
            "total_time_secs": 0.0,
            "best_node": None,
        }
