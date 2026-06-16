from __future__ import annotations

import json
import math
import random
import uuid
from enum import Enum
from pathlib import Path
from typing import Any, Callable

import numpy as np
from loguru import logger


class SamplingPolicy(str, Enum):
    UCB1 = "ucb1"
    RANDOM = "random"
    GREEDY = "greedy"
    MAP_ELITES = "map_elites"


class EvolutionNode:
    def __init__(
        self,
        program: str,
        motivation: str = "",
        results: dict[str, Any] | None = None,
        analysis: str = "",
        score: float = 0.0,
        metadata: dict[str, Any] | None = None,
        node_id: str | None = None,
    ) -> None:
        self.id = node_id or str(uuid.uuid4())[:8]
        self.program = program
        self.motivation = motivation
        self.results = results or {}
        self.analysis = analysis
        self.score = score
        self.metadata = metadata or {}
        self.visit_count: int = 0

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "program": self.program,
            "motivation": self.motivation,
            "results": self.results,
            "analysis": self.analysis,
            "score": self.score,
            "metadata": self.metadata,
            "visit_count": self.visit_count,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "EvolutionNode":
        node = cls(
            program=data.get("program", ""),
            motivation=data.get("motivation", ""),
            results=data.get("results", {}),
            analysis=data.get("analysis", ""),
            score=data.get("score", 0.0),
            metadata=data.get("metadata", {}),
            node_id=data.get("id"),
        )
        node.visit_count = data.get("visit_count", 0)
        return node


class EvolutionDatabase:
    def __init__(self, persist_path: str | Path = "./evolution_db.json") -> None:
        self.persist_path = Path(persist_path)
        self.nodes: dict[str, EvolutionNode] = {}
        self._load()

    def add_node(self, node: EvolutionNode) -> None:
        self.nodes[node.id] = node

    def get_node(self, node_id: str) -> EvolutionNode | None:
        return self.nodes.get(node_id)

    def sample(
        self,
        n: int = 5,
        policy: SamplingPolicy = SamplingPolicy.GREEDY,
        exploration_weight: float = 1.0,
        parent_id: str | None = None,
    ) -> list[EvolutionNode]:
        if not self.nodes:
            return []

        if policy == SamplingPolicy.RANDOM:
            return random.sample(list(self.nodes.values()), min(n, len(self.nodes)))

        elif policy == SamplingPolicy.GREEDY:
            sorted_nodes = sorted(self.nodes.values(), key=lambda x: x.score, reverse=True)
            return sorted_nodes[:n]

        elif policy == SamplingPolicy.UCB1:
            return self._ucb1_sample(n, exploration_weight)

        elif policy == SamplingPolicy.MAP_ELITES:
            return self._map_elites_sample(n)

        else:
            return random.sample(list(self.nodes.values()), min(n, len(self.nodes)))

    def _ucb1_sample(self, n: int, c: float) -> list[EvolutionNode]:
        total_visits = sum(node.visit_count for node in self.nodes.values()) + 1
        scored_nodes: list[tuple[EvolutionNode, float]] = []
        for node in self.nodes.values():
            if node.visit_count == 0:
                ucb = float("inf")
            else:
                exploitation = node.score
                exploration = c * math.sqrt(math.log(total_visits) / node.visit_count)
                ucb = exploitation + exploration
            scored_nodes.append((node, ucb))
        scored_nodes.sort(key=lambda x: x[1], reverse=True)
        selected = [node for node, _ in scored_nodes[:n]]
        for node in selected:
            node.visit_count += 1
        return selected

    def _map_elites_sample(self, n: int, n_bins: int = 5) -> list[EvolutionNode]:
        nodes = list(self.nodes.values())
        if len(nodes) <= n:
            return nodes

        scores = [node.score for node in nodes]
        score_min, score_max = min(scores), max(scores)
        score_range = max(score_max - score_min, 1e-6)
        bin_width = score_range / n_bins

        bins: dict[int, list[EvolutionNode]] = {}
        for node in nodes:
            bin_idx = min(int((node.score - score_min) / bin_width), n_bins - 1)
            bins.setdefault(bin_idx, []).append(node)

        selected: list[EvolutionNode] = []
        for bin_idx in range(n_bins):
            if bin_idx not in bins:
                continue
            bin_nodes = bins[bin_idx]
            best = max(bin_nodes, key=lambda x: x.score)
            if best not in selected:
                selected.append(best)

        if len(selected) < n:
            for bin_idx in range(n_bins):
                if bin_idx not in bins:
                    continue
                remaining = [x for x in bins[bin_idx] if x not in selected]
                needed = n - len(selected)
                if remaining and needed > 0:
                    selected.extend(random.sample(remaining, min(needed, len(remaining))))

        return selected[:n]

    def top_k(self, k: int = 10) -> list[EvolutionNode]:
        return sorted(self.nodes.values(), key=lambda x: x.score, reverse=True)[:k]

    def best(self) -> EvolutionNode | None:
        return self.top_k(1)[0] if self.nodes else None

    def count(self) -> int:
        return len(self.nodes)

    def stats(self) -> dict[str, Any]:
        scores = [n.score for n in self.nodes.values()]
        return {
            "total_nodes": len(self.nodes),
            "mean_score": float(np.mean(scores)) if scores else 0.0,
            "max_score": max(scores) if scores else 0.0,
            "min_score": min(scores) if scores else 0.0,
            "std_score": float(np.std(scores)) if scores else 0.0,
        }

    def prune_bottom(self, keep_top: int = 50) -> None:
        if len(self.nodes) <= keep_top:
            return
        sorted_nodes = sorted(self.nodes.values(), key=lambda x: x.score, reverse=True)
        keep_ids = {node.id for node in sorted_nodes[:keep_top]}
        self.nodes = {k: v for k, v in self.nodes.items() if k in keep_ids}
        logger.info(f"Database pruned to top {keep_top} nodes")

    def save(self) -> None:
        data = {node_id: node.to_dict() for node_id, node in self.nodes.items()}
        self.persist_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.persist_path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        logger.debug(f"EvolutionDatabase saved: {len(self.nodes)} nodes")

    def _load(self) -> None:
        if not self.persist_path.exists():
            return
        try:
            with open(self.persist_path, encoding="utf-8") as f:
                data = json.load(f)
            for node_id, node_data in data.items():
                self.nodes[node_id] = EvolutionNode.from_dict(node_data)
            logger.info(f"EvolutionDatabase loaded: {len(self.nodes)} nodes")
        except Exception as e:
            logger.warning(f"Could not load evolution database: {e}")

    def export_summary(self) -> list[dict[str, Any]]:
        return [
            {
                "id": node.id,
                "score": node.score,
                "motivation": node.motivation[:200],
                "visit_count": node.visit_count,
            }
            for node in sorted(self.nodes.values(), key=lambda x: x.score, reverse=True)
        ]
