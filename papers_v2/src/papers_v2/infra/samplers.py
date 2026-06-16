"""
Sampling algorithms - adaptes de l'implementation ASI-Evolve.
UCB1, Greedy, Random, Island (MAP-Elites).
"""

from __future__ import annotations

import math
import random
from abc import ABC, abstractmethod
from typing import Any


class BaseSampler(ABC):
    @abstractmethod
    def sample(self, nodes: list[Any], n: int) -> list[Any]:
        ...


class GreedySampler(BaseSampler):
    def sample(self, nodes: list[Any], n: int) -> list[Any]:
        if not nodes:
            return []
        return sorted(nodes, key=lambda x: getattr(x, "score", 0), reverse=True)[:n]


class RandomSampler(BaseSampler):
    def sample(self, nodes: list[Any], n: int) -> list[Any]:
        if not nodes:
            return []
        return random.sample(nodes, min(n, len(nodes)))


class UCB1Sampler(BaseSampler):
    def __init__(self, c: float = 1.414) -> None:
        self.c = c

    def sample(self, nodes: list[Any], n: int) -> list[Any]:
        if not nodes:
            return []
        n = min(n, len(nodes))
        scored = [node for node in nodes if getattr(node, "visit_count", 0) > 0]
        scored_nodes = [node for node in nodes if getattr(node, "visit_count", 0) > 0]
        if not scored_nodes:
            return random.sample(nodes, n)

        scores_list = [getattr(node, "score", 0) for node in scored_nodes]
        min_s, max_s = min(scores_list), max(scores_list)
        score_range = max_s - min_s if max_s != min_s else 1.0
        total_visits = sum(getattr(node, "visit_count", 0) for node in nodes)

        ucb_values = []
        for node in nodes:
            vc = getattr(node, "visit_count", 0)
            if vc == 0:
                ucb_values.append((node, float("inf")))
            else:
                ns = (getattr(node, "score", 0) - min_s) / score_range
                ex = self.c * math.sqrt(math.log(max(total_visits, 1)) / vc)
                ucb_values.append((node, ns + ex))

        ucb_values.sort(key=lambda x: x[1], reverse=True)
        selected = [node for node, _ in ucb_values[:n]]
        for node in selected:
            node.visit_count = getattr(node, "visit_count", 0) + 1
        return selected


class IslandSampler(BaseSampler):
    def __init__(
        self,
        num_islands: int = 5,
        migration_interval: int = 10,
        migration_rate: float = 0.1,
        exploration_ratio: float = 0.2,
        exploitation_ratio: float = 0.3,
    ) -> None:
        self.num_islands = num_islands
        self.migration_interval = migration_interval
        self.migration_rate = migration_rate
        self.exploration_ratio = exploration_ratio
        self.exploitation_ratio = exploitation_ratio
        self.islands: list[set[int]] = [set() for _ in range(num_islands)]
        self.island_best: list[int | None] = [None] * num_islands
        self.generations: list[int] = [0] * num_islands
        self.current_island = 0
        self.archive: set[int] = set()
        self._total_generations = 0

    def sample(self, nodes: list[Any], n: int) -> list[Any]:
        if not nodes:
            return []

        n = min(n, len(nodes))

        if self._total_generations > 0 and self._total_generations % self.migration_interval == 0:
            self._migrate(nodes)

        island_id = self.current_island
        island_nodes = self._get_island_nodes(island_id, nodes)
        if not island_nodes:
            island_nodes = nodes

        self.generations[island_id] += 1
        self._total_generations += 1
        self.current_island = (self.current_island + 1) % self.num_islands

        selected = []
        for _ in range(n):
            r = random.random()
            if r < self.exploration_ratio:
                node = random.choice(island_nodes)
            elif r < self.exploration_ratio + self.exploitation_ratio:
                arch_nodes = [n for n in nodes if getattr(n, "id", None) in self.archive]
                node = random.choice(arch_nodes) if arch_nodes else max(island_nodes, key=lambda x: getattr(x, "score", 0))
            else:
                node = max(island_nodes, key=lambda x: getattr(x, "score", 0))

            if node not in selected:
                selected.append(node)
                node.visit_count = getattr(node, "visit_count", 0) + 1

        return selected

    def _get_island_nodes(self, island_id: int, nodes: list[Any]) -> list[Any]:
        island_ids = self.islands[island_id]
        return [n for n in nodes if getattr(n, "id", None) in island_ids]

    def _migrate(self, nodes: list[Any]) -> None:
        for i in range(self.num_islands):
            island_nodes = [(n, getattr(n, "score", 0)) for n in nodes if getattr(n, "id", None) in self.islands[i]]
            if not island_nodes:
                continue
            island_nodes.sort(key=lambda x: x[1], reverse=True)
            migrants = island_nodes[:max(1, int(len(island_nodes) * self.migration_rate))]
            next_island = (i + 1) % self.num_islands
            for n, _ in migrants:
                self.islands[next_island].add(getattr(n, "id", None))

    def on_node_added(self, node: Any) -> None:
        node_id = getattr(node, "id", None)
        if node_id is None:
            return
        island_id = node.metadata.get("island", self.current_island) if hasattr(node, "metadata") else self.current_island
        island_id = island_id % self.num_islands
        self.islands[island_id].add(node_id)
        score = getattr(node, "score", 0)
        current_best = self.island_best[island_id]
        if current_best is None or score > getattr(node, "score", 0):
            self.island_best[island_id] = node_id
        if score > 0.5:
            self.archive.add(node_id)
            if len(self.archive) > 100:
                self.archive.pop()


def get_sampler(name: str = "greedy", **kwargs: Any) -> BaseSampler:
    samplers = {
        "greedy": GreedySampler,
        "random": RandomSampler,
        "ucb1": UCB1Sampler,
        "island": IslandSampler,
        "map_elites": IslandSampler,
    }
    cls = samplers.get(name, GreedySampler)
    if name == "ucb1":
        return cls(c=kwargs.get("c", 1.414))
    elif name in ("island", "map_elites"):
        return cls(
            num_islands=kwargs.get("num_islands", 5),
            migration_interval=kwargs.get("migration_interval", 10),
            migration_rate=kwargs.get("migration_rate", 0.1),
            exploration_ratio=kwargs.get("exploration_ratio", 0.2),
            exploitation_ratio=kwargs.get("exploitation_ratio", 0.3),
        )
    return cls()
