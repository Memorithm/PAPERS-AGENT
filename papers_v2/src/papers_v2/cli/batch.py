from __future__ import annotations

from pathlib import Path
from typing import Any

import typer
from loguru import logger
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.table import Table

from papers_v2.core.models import AnalysisReport, Publication
from papers_v2.core.orchestrator import PapersEngine
from papers_v2.extraction.external_sources import HuggingFacePapersExtractor
from papers_v2.knowledge.graph import KnowledgeGraph
from papers_v2.knowledge.vector_store import PaperVectorStore
from papers_v2.utils.reporting import ReportRenderer

console = Console()


def run_batch_analysis(
    publications: list[Publication],
    output_dir: Path,
    knowledge_graph: Path,
    vector_store: Path,
    use_llm: bool = True,
    llm_model: str = "gemma4:e2b",
    deep: bool = False,
) -> list[AnalysisReport]:
    output_dir.mkdir(parents=True, exist_ok=True)
    kg = KnowledgeGraph(str(knowledge_graph))
    vs = PaperVectorStore(str(vector_store))
    engine = PapersEngine(use_llm=use_llm, llm_model=llm_model)
    renderer = ReportRenderer()

    reports: list[AnalysisReport] = []
    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        console=console,
    ) as progress:
        for pub in publications:
            task = progress.add_task(f"Analyse de {pub.title[:50]}...", total=None)
            try:
                report = engine.analyze(pub, deep=deep)
                reports.append(report)

                safe_title = "".join(c if c.isalnum() else "_" for c in pub.title[:40])
                md_path = output_dir / f"{report.publication.id}_{safe_title}.md"
                renderer.save(report, str(md_path))

                kg.add_report(report)
                vs.add_report(report)
                progress.update(task, description=f"[green]✓ {pub.title[:50]}")
            except Exception as e:
                logger.error(f"Échec analyse {pub.title}: {e}")
                progress.update(task, description=f"[red]✗ {pub.title[:50]}")
            finally:
                progress.remove_task(task)

    kg.save()
    vs.persist()
    return reports


def display_batch_summary(reports: list[AnalysisReport]) -> None:
    table = Table(title="Résumé du batch d'analyse")
    table.add_column("Titre", style="magenta", no_wrap=False)
    table.add_column("Score Intégration", style="cyan")
    table.add_column("Recommandation", style="green")

    for r in reports:
        table.add_row(
            r.publication.title[:50],
            f"{r.integration.score:.2f}",
            r.recommendation.value,
        )

    console.print(table)
