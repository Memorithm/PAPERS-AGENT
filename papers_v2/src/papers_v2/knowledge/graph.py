from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import networkx as nx
from loguru import logger

from papers_v2.core.models import AnalysisReport


class KnowledgeGraph:
    def __init__(self, path: str | Path = "./knowledge_graph.json") -> None:
        self.path = Path(path)
        self.graph = nx.DiGraph()
        self._load()

    def _load(self) -> None:
        if self.path.exists():
            try:
                data = json.loads(self.path.read_text(encoding="utf-8"))
                self.graph = nx.node_link_graph(data)
            except Exception as e:
                logger.warning(f"Impossible de charger le graphe: {e}")

    def add_report(self, report: AnalysisReport) -> None:
        paper_node = f"paper:{report.publication.id}"
        self.graph.add_node(paper_node, kind="paper", title=report.publication.title)

        for contrib in report.scientific_contributions:
            node_id = f"contrib:{hash(contrib) & 0xFFFFFFFF:08x}"
            self.graph.add_node(node_id, kind="contribution", text=contrib)
            self.graph.add_edge(paper_node, node_id)

        for module in report.architectural_mapping.impacted_modules:
            node_id = f"module:{module}"
            self.graph.add_node(node_id, kind="module", name=module)
            self.graph.add_edge(paper_node, node_id)

    def save(self) -> None:
        data = nx.node_link_data(self.graph)
        self.path.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")

    def find_similar(self, report: AnalysisReport, top_k: int = 5) -> list[dict[str, Any]]:
        results = []
        for node, attrs in self.graph.nodes(data=True):
            if attrs.get("kind") == "paper" and node != f"paper:{report.publication.id}":
                results.append({"id": node, "title": attrs.get("title", "")})
        return results[:top_k]
