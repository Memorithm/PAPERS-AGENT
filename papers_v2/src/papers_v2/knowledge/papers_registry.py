from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from loguru import logger
from rich.console import Console
from rich.table import Table

from papers_v2.core.models import Author, Domain, MathematicalAnalysis, Publication
from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.knowledge.graph import KnowledgeGraph
from papers_v2.knowledge.vector_store import PaperVectorStore

console = Console()


class PaperRegistry:
    def __init__(
        self,
        registry_path: str | Path = "./papers_registry.json",
        knowledge_graph_path: str | Path = "./knowledge_graph.json",
        vector_store_path: str | Path = "./vector_store",
        cognition_path: str | Path = "./cognition_store",
    ) -> None:
        self.registry_path = Path(registry_path)
        self.kg_path = Path(knowledge_graph_path)
        self.vs_path = Path(vector_store_path)
        self.cog_path = Path(cognition_path)

        self.papers: list[dict[str, Any]] = []
        self._load()

    def _load(self) -> None:
        if self.registry_path.exists():
            with open(self.registry_path, encoding="utf-8") as f:
                self.papers = json.load(f)
            logger.info(f"PaperRegistry loaded: {len(self.papers)} papers")

    def save(self) -> None:
        with open(self.registry_path, "w", encoding="utf-8") as f:
            json.dump(self.papers, f, ensure_ascii=False, indent=2)

    def add_paper(self, paper: dict[str, Any]) -> None:
        if not any(p.get("id") == paper.get("id") for p in self.papers):
            self.papers.append(paper)
            self.save()

    def get_paper(self, paper_id: str) -> dict[str, Any] | None:
        for p in self.papers:
            if p.get("id") == paper_id:
                return p
        return None

    def search(self, query: str) -> list[dict[str, Any]]:
        q = query.lower()
        results = []
        for p in self.papers:
            text = (
                p.get("title", "").lower()
                + " "
                + " ".join(p.get("tags", [])).lower()
                + " "
                + p.get("abstract", "").lower()
                + " "
                + " ".join(p.get("authors", [])).lower()
            )
            if q in text:
                results.append(p)
        return results

    def filter_by_tag(self, tag: str) -> list[dict[str, Any]]:
        return [p for p in self.papers if tag in p.get("tags", [])]

    def filter_by_domain(self, domain: str) -> list[dict[str, Any]]:
        return [p for p in self.papers if p.get("domain") == domain]

    def filter_by_year(self, year: int) -> list[dict[str, Any]]:
        return [p for p in self.papers if p.get("year") == year]

    def list_tags(self) -> list[str]:
        tags: set[str] = set()
        for p in self.papers:
            tags.update(p.get("tags", []))
        return sorted(tags)

    def list_domains(self) -> list[str]:
        domains: set[str] = set()
        for p in self.papers:
            d = p.get("domain", "")
            if d:
                domains.add(d)
        return sorted(domains)

    def stats(self) -> dict[str, Any]:
        years = {}
        domains = {}
        for p in self.papers:
            y = p.get("year", 0)
            years[y] = years.get(y, 0) + 1
            d = p.get("domain", "unknown")
            domains[d] = domains.get(d, 0) + 1
        return {
            "total_papers": len(self.papers),
            "by_year": years,
            "by_domain": domains,
            "total_tags": len(self.list_tags()),
        }

    def import_to_cognition_base(
        self,
        cognition: CognitionBase | None = None,
    ) -> int:
        ct = cognition or CognitionBase(persist_dir=self.cog_path)
        count = 0
        for paper in self.papers:
            entry_id = f"reg-{paper['id']}"

            content_parts = [
                f"Title: {paper['title']}",
                f"Authors: {', '.join(paper.get('authors', []))}",
                f"Key Insight: {paper.get('key_insight', '')}",
                paper.get("abstract", ""),
            ]
            content = "\n\n".join(content_parts)

            entry = CognitionEntry(
                content=content,
                source=paper.get("source", paper.get("title", "")),
                entry_type="paper_insight",
                tags=paper.get("tags", []),
                entry_id=entry_id,
            )
            ct.add_entry(entry)
            count += 1
        logger.info(f"Imported {count} papers to CognitionBase")
        return count

    def import_to_knowledge_graph(
        self,
        kg: KnowledgeGraph | None = None,
    ) -> int:
        g = kg or KnowledgeGraph(path=str(self.kg_path))
        count = 0
        for paper in self.papers:
            paper_node = f"paper:{paper['id']}"
            g.graph.add_node(
                paper_node,
                kind="paper",
                title=paper["title"],
                year=paper.get("year"),
                source=paper.get("source", ""),
                url=paper.get("url", ""),
                relevance=paper.get("relevance_score", 0.0),
            )

            for tag in paper.get("tags", []):
                tag_node = f"tag:{tag}"
                g.graph.add_node(tag_node, kind="tag", name=tag)
                g.graph.add_edge(paper_node, tag_node, relation="has_tag")

            domain = paper.get("domain", "")
            if domain:
                domain_node = f"domain:{domain}"
                g.graph.add_node(domain_node, kind="domain", name=domain)
                g.graph.add_edge(paper_node, domain_node, relation="belongs_to")

            insight = paper.get("key_insight", "")
            if insight:
                insight_node = f"insight:{paper['id']}"
                g.graph.add_node(insight_node, kind="insight", text=insight)
                g.graph.add_edge(paper_node, insight_node, relation="has_insight")

            count += 1

        g.save()
        logger.info(f"Imported {count} papers to KnowledgeGraph")
        return count

    def import_to_vector_store(
        self,
        vs: PaperVectorStore | None = None,
    ) -> int:
        v = vs or PaperVectorStore(persist_dir=self.vs_path)
        count = 0
        from papers_v2.core.models import AnalysisReport, Publication

        for paper in self.papers:
            pub = Publication(
                id=f"registry:{paper['id']}",
                title=paper["title"],
                authors=[Author(name=a) for a in paper.get("authors", [])],
                source=paper.get("source", ""),
                paper_url=paper.get("url", ""),
                abstract=paper.get("abstract", ""),
                domains=[Domain.parse(paper.get("domain", ""))] if paper.get("domain") else [],
            )
            report = AnalysisReport(publication=pub)
            report.scientific_contributions = [paper.get("key_insight", "")]
            v.add_report(report)
            count += 1

        v.persist()
        logger.info(f"Imported {count} papers to VectorStore")
        return count

    def import_all(
        self,
        cognition: CognitionBase | None = None,
        kg: KnowledgeGraph | None = None,
        vs: PaperVectorStore | None = None,
    ) -> dict[str, int]:
        results = {}
        results["cognition"] = self.import_to_cognition_base(cognition)
        results["knowledge_graph"] = self.import_to_knowledge_graph(kg)
        results["vector_store"] = self.import_to_vector_store(vs)
        return results

    def display_table(
        self,
        papers: list[dict[str, Any]] | None = None,
        title: str = "Paper Registry",
        limit: int = 50,
    ) -> None:
        items = papers or self.papers
        table = Table(title=title)
        table.add_column("ID", style="cyan", no_wrap=True)
        table.add_column("Title", style="magenta", no_wrap=False)
        table.add_column("Year", style="green")
        table.add_column("Score", style="yellow")
        table.add_column("Tags", style="blue", no_wrap=False)

        for p in items[:limit]:
            tags_str = ", ".join(p.get("tags", [])[:4])
            if len(p.get("tags", [])) > 4:
                tags_str += "..."
            table.add_row(
                p.get("id", ""),
                p.get("title", "")[:80],
                str(p.get("year", "")),
                f"{p.get('relevance_score', 0):.2f}",
                tags_str,
            )
        console.print(table)
