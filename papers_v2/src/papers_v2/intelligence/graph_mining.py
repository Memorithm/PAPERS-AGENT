"""
Graph Pattern Miner
Based on: Odin (Kizito et al. 2026) - Multi-signal graph intelligence
         GRiD (Cheng et al. 2026) - Graph-like logical rules via diffusion
         GNN Activation Patterns (Tori et al. 2026)

Discovers meaningful patterns in knowledge graphs without prior specification.
Multi-signal scoring: structural + semantic + temporal + relational.
"""

from __future__ import annotations

import math
from collections import defaultdict
from typing import Any

import networkx as nx
import numpy as np
from loguru import logger


class GraphPattern:
    def __init__(
        self,
        pattern_id: str,
        pattern_type: str,
        nodes: list[str],
        edges: list[tuple[str, str]],
        description: str,
        significance: float = 0.0,
        frequency: int = 1,
    ) -> None:
        self.id = pattern_id
        self.type = pattern_type
        self.nodes = nodes
        self.edges = edges
        self.description = description
        self.significance = significance
        self.frequency = frequency

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "type": self.type,
            "nodes": self.nodes,
            "edges": [list(e) for e in self.edges],
            "description": self.description,
            "significance": self.significance,
            "frequency": self.frequency,
        }


class GraphPatternMiner:
    def __init__(self, graph: nx.DiGraph | None = None) -> None:
        self.graph = graph or nx.DiGraph()
        self.patterns: dict[str, GraphPattern] = {}
        self.node_types: dict[str, str] = {}

    def set_graph(self, graph: nx.DiGraph) -> None:
        self.graph = graph
        for node, attrs in graph.nodes(data=True):
            self.node_types[node] = attrs.get("kind", "unknown")

    def mine_patterns(self, min_significance: float = 0.1) -> list[GraphPattern]:
        patterns: list[GraphPattern] = []
        patterns.extend(self._mine_star_patterns())
        patterns.extend(self._mine_chain_patterns())
        patterns.extend(self._mine_cycle_patterns())
        patterns.extend(self._mine_bridge_patterns())
        patterns.extend(self._mine_community_patterns())

        significant = [
            p for p in patterns if p.significance >= min_significance
        ]
        for p in significant:
            self.patterns[p.id] = p

        logger.info(f"GraphPatternMiner: {len(significant)} significant patterns found")
        return significant

    def _mine_star_patterns(self) -> list[GraphPattern]:
        patterns: list[GraphPattern] = []
        degrees = dict(self.graph.degree())

        for node, deg in degrees.items():
            if deg >= 3:
                neighbors = list(self.graph.neighbors(node))
                edges = [(node, n) for n in neighbors]

                node_kind = self.node_types.get(node, "unknown")
                neighbor_kinds = [
                    self.node_types.get(n, "unknown") for n in neighbors
                ]

                structural_score = min(1.0, math.log(deg + 1) / math.log(10))
                type_diversity = len(set(neighbor_kinds)) / max(len(neighbor_kinds), 1)
                significance = 0.6 * structural_score + 0.4 * type_diversity

                patterns.append(GraphPattern(
                    pattern_id=f"star_{node[:16]}",
                    pattern_type="star",
                    nodes=[node] + neighbors[:10],
                    edges=edges[:10],
                    description=(
                        f"Star pattern centered on {node_kind} node '{node[:20]}' "
                        f"connecting to {deg} nodes of types {set(neighbor_kinds)}"
                    ),
                    significance=significance,
                    frequency=deg,
                ))

        return sorted(patterns, key=lambda p: p.significance, reverse=True)[:10]

    def _mine_chain_patterns(self, max_length: int = 4) -> list[GraphPattern]:
        patterns: list[GraphPattern] = []
        chains_by_length: dict[int, list[list[str]]] = defaultdict(list)

        for node in self.graph.nodes():
            self._dfs_chains(node, [], set(), max_length, chains_by_length)

        for length, chains in chains_by_length.items():
            if len(chains) < 2:
                continue

            kind_chains: dict[str, int] = defaultdict(int)
            for chain in chains:
                kind_seq = " -> ".join(
                    self.node_types.get(n, "?") for n in chain
                )
                kind_chains[kind_seq] += 1

            for kind_seq, freq in kind_chains.items():
                if freq >= 2:
                    significance = float(freq) / max(len(chains), 1)
                    patterns.append(GraphPattern(
                        pattern_id=f"chain_{hash(kind_seq) % 100000:05d}",
                        pattern_type="chain",
                        nodes=[],
                        edges=[],
                        description=f"Chain pattern length {length}: {kind_seq} (found {freq}x)",
                        significance=significance,
                        frequency=freq,
                    ))

        return patterns

    def _dfs_chains(
        self,
        node: str,
        path: list[str],
        visited: set[str],
        max_length: int,
        chains_by_length: dict[int, list[list[str]]],
    ) -> None:
        if len(path) >= max_length:
            return
        path = path + [node]
        visited = visited | {node}
        if len(path) >= 2:
            chains_by_length[len(path)].append(path)

        for neighbor in self.graph.neighbors(node):
            if neighbor not in visited:
                self._dfs_chains(
                    neighbor, path, visited, max_length, chains_by_length,
                )

    def _mine_cycle_patterns(self) -> list[GraphPattern]:
        patterns: list[GraphPattern] = []
        try:
            cycles = list(nx.simple_cycles(self.graph))
        except Exception:
            cycles = []

        for cycle in cycles[:20]:
            if 3 <= len(cycle) <= 6:
                kind_cycle = [
                    self.node_types.get(n, "unknown") for n in cycle
                ]
                significance = 1.0 / len(cycle)
                patterns.append(GraphPattern(
                    pattern_id=f"cycle_{hash(str(cycle)) % 100000:05d}",
                    pattern_type="cycle",
                    nodes=cycle,
                    edges=[(cycle[i], cycle[(i + 1) % len(cycle)]) for i in range(len(cycle))],
                    description=(
                        f"Cycle of length {len(cycle)} with types {kind_cycle}"
                    ),
                    significance=significance,
                    frequency=1,
                ))

        return patterns

    def _mine_bridge_patterns(self) -> list[GraphPattern]:
        patterns: list[GraphPattern] = []

        if not nx.is_directed(self.graph):
            undirected = self.graph
        else:
            undirected = self.graph.to_undirected()

        try:
            bridges = list(nx.bridges(undirected))
        except Exception:
            return patterns

        for u, v in bridges[:10]:
            u_kind = self.node_types.get(u, "unknown")
            v_kind = self.node_types.get(v, "unknown")
            patterns.append(GraphPattern(
                pattern_id=f"bridge_{u[:8]}_{v[:8]}",
                pattern_type="bridge",
                nodes=[u, v],
                edges=[(u, v)],
                description=f"Bridge connecting {u_kind} and {v_kind} subgraphs",
                significance=0.7,
                frequency=1,
            ))

        return patterns

    def _mine_community_patterns(self) -> list[GraphPattern]:
        patterns: list[GraphPattern] = []
        if len(self.graph.nodes()) < 5:
            return patterns

        undirected = (
            self.graph.to_undirected()
            if nx.is_directed(self.graph)
            else self.graph
        )

        try:
            communities = nx.community.louvain_communities(undirected)
        except Exception:
            return patterns

        for i, community in enumerate(communities[:5]):
            if len(community) < 2:
                continue
            kinds = [
                self.node_types.get(n, "unknown") for n in community
            ]
            kind_counts = defaultdict(int)
            for k in kinds:
                kind_counts[k] += 1

            significance = len(community) / max(self.graph.number_of_nodes(), 1)
            patterns.append(GraphPattern(
                pattern_id=f"community_{i}",
                pattern_type="community",
                nodes=list(community),
                edges=[],
                description=(
                    f"Community of {len(community)} nodes: "
                    f"{dict(kind_counts)}"
                ),
                significance=significance,
                frequency=len(community),
            ))

        return patterns

    def compute_compass_score(
        self,
        pattern: GraphPattern,
        semantic_scores: dict[str, float] | None = None,
        temporal_weights: dict[str, float] | None = None,
    ) -> float:
        structural = pattern.significance

        semantic = 0.5
        if semantic_scores:
            node_scores = [
                semantic_scores.get(n, 0.5) for n in pattern.nodes
            ]
            semantic = float(np.mean(node_scores)) if node_scores else 0.5

        temporal = 0.5
        if temporal_weights:
            node_weights = [
                temporal_weights.get(n, 0.5) for n in pattern.nodes
            ]
            temporal = float(np.mean(node_weights)) if node_weights else 0.5

        bridge_boost = 0.0
        if pattern.type == "bridge":
            bridge_boost = 0.2

        compass = (
            0.25 * structural
            + 0.30 * semantic
            + 0.20 * temporal
            + 0.25 * (1.0 if pattern.type in ("star", "bridge") else 0.5)
            + bridge_boost
        )

        return min(1.0, max(0.0, compass))

    def query_by_type(self, pattern_type: str) -> list[GraphPattern]:
        return [p for p in self.patterns.values() if p.type == pattern_type]

    def query_by_node(self, node_id: str) -> list[GraphPattern]:
        return [
            p for p in self.patterns.values()
            if node_id in p.nodes
        ]

    def top_patterns(self, k: int = 10, by: str = "significance") -> list[GraphPattern]:
        return sorted(
            self.patterns.values(),
            key=lambda p: getattr(p, by, 0),
            reverse=True,
        )[:k]

    def export_report(self) -> dict[str, Any]:
        return {
            "total_patterns": len(self.patterns),
            "by_type": {
                t: len(self.query_by_type(t))
                for t in {"star", "chain", "cycle", "bridge", "community"}
            },
            "top_significant": [p.to_dict() for p in self.top_patterns(5)],
            "graph_stats": {
                "nodes": self.graph.number_of_nodes(),
                "edges": self.graph.number_of_edges(),
            },
        }
