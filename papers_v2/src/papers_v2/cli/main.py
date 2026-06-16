from __future__ import annotations

from pathlib import Path

import typer
from loguru import logger
from rich.console import Console
from rich.table import Table

from papers_v2.cli.batch import display_batch_summary, run_batch_analysis
from papers_v2.core.models import AnalysisReport
from papers_v2.core.orchestrator import PapersEngine
from papers_v2.extraction.extractors import ExtractionPipeline
from papers_v2.extraction.external_sources import HuggingFacePapersExtractor, SemanticScholarExtractor
from papers_v2.knowledge.graph import KnowledgeGraph
from papers_v2.knowledge.server import serve as serve_app
from papers_v2.knowledge.vector_store import PaperVectorStore
from papers_v2.utils.reporting import ReportRenderer

app = typer.Typer(help="PAPERS V2 - Predictive Academic Paper Extraction & Engineering Research System")
console = Console()


@app.command()
def analyze(
    source: str = typer.Argument(..., help="URL ArXiv, chemin PDF, ou fichier texte"),
    output: str = typer.Option("./output/report.md", "--output", "-o", help="Chemin du rapport de sortie"),
    knowledge_graph: str = typer.Option("./knowledge_graph.json", "--kg", help="Chemin du graphe de connaissances"),
    vector_store: str = typer.Option("./vector_store", "--vs", help="Chemin du store vectoriel"),
    use_llm: bool = typer.Option(True, "--llm/--no-llm", help="Utiliser le LLM local Ollama"),
    llm_model: str = typer.Option("gemma4:e2b", "--llm-model", help="Modèle Ollama à utiliser"),
    deep: bool = typer.Option(False, "--deep", help="Activer l'analyse approfondie multi-passes"),
    html: bool = typer.Option(False, "--html", help="Générer aussi un rapport HTML"),
    json_output: bool = typer.Option(False, "--json", help="Générer aussi un rapport JSON"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Mode verbeux"),
) -> None:
    """Analyse une publication et génère un rapport complet."""
    if verbose:
        logger.enable("papers_v2")

    pipeline = ExtractionPipeline()
    engine = PapersEngine(use_llm=use_llm, llm_model=llm_model)
    renderer = ReportRenderer()
    kg = KnowledgeGraph(knowledge_graph)
    vs = PaperVectorStore(vector_store)

    try:
        publication = pipeline.extract(source)
        report = engine.analyze(publication, deep=deep)
        kg.add_report(report)
        kg.save()
        vs.add_report(report)
        vs.persist()

        output_path = Path(output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        renderer.save(report, output)

        if html:
            from papers_v2.llm.analyzer import LLMAnalyzer
            html_content = LLMAnalyzer(model=llm_model).generate_html_summary(report)
            html_path = output_path.with_suffix(".html")
            html_path.write_text(html_content, encoding="utf-8")
            console.print(f"[green]Rapport HTML : {html_path}[/green]")

        if json_output:
            json_path = output_path.with_suffix(".json")
            json_path.write_text(report.model_dump_json(indent=2), encoding="utf-8")
            console.print(f"[green]Rapport JSON : {json_path}[/green]")

        _display_summary(report)
        console.print(f"\n[green]Rapport sauvegardé : {output}[/green]")

    except Exception as e:
        console.print(f"[red]Erreur : {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def batch(
    source: str = typer.Option("huggingface", "--source", help="Source: huggingface|semantic_scholar|file"),
    query: str = typer.Option("", "--query", help="Requête pour semantic_scholar"),
    limit: int = typer.Option(5, "--limit", "-n", help="Nombre de papiers à analyser"),
    output_dir: str = typer.Option("./output/batch", "--output-dir", help="Répertoire de sortie"),
    knowledge_graph: str = typer.Option("./knowledge_graph.json", "--kg", help="Chemin du graphe de connaissances"),
    vector_store: str = typer.Option("./vector_store", "--vs", help="Chemin du store vectoriel"),
    use_llm: bool = typer.Option(True, "--llm/--no-llm", help="Utiliser le LLM local Ollama"),
    llm_model: str = typer.Option("gemma4:e2b", "--llm-model", help="Modèle Ollama à utiliser"),
    deep: bool = typer.Option(False, "--deep", help="Activer l'analyse approfondie multi-passes"),
) -> None:
    """Analyse automatiquement plusieurs publications."""
    publications = []
    if source == "huggingface":
        extractor = HuggingFacePapersExtractor()
        publications = extractor.latest(limit=limit)
    elif source == "semantic_scholar":
        if not query:
            console.print("[red]--query requis pour semantic_scholar[/red]")
            raise typer.Exit(code=1)
        extractor = SemanticScholarExtractor()
        publications = extractor.search(query, limit=limit)
    else:
        console.print(f"[red]Source non supportée: {source}[/red]")
        raise typer.Exit(code=1)

    reports = run_batch_analysis(
        publications=publications,
        output_dir=Path(output_dir),
        knowledge_graph=Path(knowledge_graph),
        vector_store=Path(vector_store),
        use_llm=use_llm,
        llm_model=llm_model,
        deep=deep,
    )
    display_batch_summary(reports)
    console.print(f"\n[green]{len(reports)} rapports générés dans {output_dir}[/green]")


@app.command()
def search(
    query: str = typer.Argument(..., help="Requête de recherche"),
    limit: int = typer.Option(5, "--limit", "-n", help="Nombre de résultats"),
    vector_store: str = typer.Option("./vector_store", "--vs", help="Chemin du store vectoriel"),
) -> None:
    """Recherche sémantique dans le graphe de connaissances."""
    vs = PaperVectorStore(vector_store)
    results = vs.search(query, n_results=limit)

    table = Table(title="Résultats de recherche sémantique")
    table.add_column("ID", style="cyan")
    table.add_column("Titre", style="magenta")
    table.add_column("Score", style="green")

    for r in results:
        table.add_row(r["id"], r["metadata"].get("title", ""), f"{r['distance']:.3f}")

    console.print(table)


@app.command()
def hf_latest(
    limit: int = typer.Option(10, "--limit", "-n", help="Nombre de papiers"),
) -> None:
    """Liste les derniers papiers mis en avant par Hugging Face."""
    extractor = HuggingFacePapersExtractor()
    papers = extractor.latest(limit=limit)
    table = Table(title="Derniers papiers Hugging Face")
    table.add_column("Titre", style="magenta", no_wrap=False)
    table.add_column("Source", style="cyan")
    for p in papers:
        table.add_row(p.title, p.paper_url or "")
    console.print(table)


@app.command()
def s2_search(
    query: str = typer.Argument(..., help="Requête Semantic Scholar"),
    limit: int = typer.Option(5, "--limit", "-n", help="Nombre de résultats"),
) -> None:
    """Recherche un papier via Semantic Scholar."""
    extractor = SemanticScholarExtractor()
    papers = extractor.search(query, limit=limit)
    table = Table(title="Résultats Semantic Scholar")
    table.add_column("Titre", style="magenta", no_wrap=False)
    table.add_column("Auteurs", style="cyan")
    for p in papers:
        table.add_row(p.title, ", ".join(a.name for a in p.authors[:3]))
    console.print(table)


@app.command()
def serve(
    host: str = typer.Option("127.0.0.1", "--host", help="Hôte du serveur"),
    port: int = typer.Option(8000, "--port", "-p", help="Port du serveur"),
) -> None:
    """Lance un serveur web pour explorer le graphe de connaissances."""
    serve_app(host=host, port=port)


# ── ASI-Evolve Commands ──────────────────────────────────────────

@app.command()
def evolve(
    config: str = typer.Option("./evolution_config.yaml", "--config", "-c", help="Fichier de configuration YAML"),
    eval_command: str = typer.Option("", "--eval-cmd", help="Commande shell d'évaluation"),
    task: str = typer.Option("", "--task", help="Description de la tâche (inline)"),
    model: str = typer.Option("gemma4:e2b", "--model", help="Modèle Ollama"),
    max_rounds: int = typer.Option(50, "--max-rounds", "-n", help="Nombre maximum de rounds"),
    target: float = typer.Option(0.0, "--target", help="Score cible pour arrêt précoce"),
    candidates: int = typer.Option(3, "--candidates", "-k", help="Candidats par round"),
    patience: int = typer.Option(10, "--patience", help="Rounds sans amélioration avant arrêt"),
    policy: str = typer.Option("greedy", "--policy", help="Politique: ucb1|random|greedy|map_elites"),
    output_dir: str = typer.Option("./evolution_output", "--output-dir", "-o", help="Répertoire de sortie"),
    db_path: str = typer.Option("./evolution_db.json", "--db", help="Base de données d'évolution"),
    cognition_path: str = typer.Option("./cognition_store", "--cognition", help="Store de cognition"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Mode verbeux"),
) -> None:
    """Lance une boucle d'évolution ASI-Evolve (learn-design-experiment-analyze)."""
    from papers_v2.core.models import EvolutionConfig
    from papers_v2.evolution import CognitionBase, EvolutionDatabase, EvolutionLoop, SamplingPolicy

    if verbose:
        logger.enable("papers_v2")

    # Load config from file, CLI args override
    if Path(config).exists():
        cfg = EvolutionConfig.load(config)
    else:
        cfg = EvolutionConfig()

    task_desc = task or cfg.task_description
    if not task_desc and not eval_command:
        console.print("[red]--task ou --eval-cmd requis[/red]")
        raise typer.Exit(code=1)

    model_name = model or cfg.model
    sampling = SamplingPolicy(policy or cfg.sampling_policy)
    rounds = max_rounds or cfg.max_rounds
    target_score = target if target > 0 else cfg.target_score
    pat = patience or cfg.patience
    n_cand = candidates or cfg.n_candidates_per_round

    cognition = CognitionBase(persist_dir=cognition_path or cfg.cognition_path)
    database = EvolutionDatabase(persist_path=db_path or cfg.db_path)

    loop = EvolutionLoop(
        task_description=task_desc,
        cognition=cognition,
        database=database,
        model=model_name,
        sampling_policy=sampling,
        max_rounds=rounds,
        target_score=target_score,
        patience=pat,
        db_path=db_path or cfg.db_path,
        cognition_path=cognition_path or cfg.cognition_path,
        output_dir=output_dir or cfg.output_dir,
    )

    console.print(f"[bold cyan]ASI-Evolve: {task_desc[:80]}...[/bold cyan]")
    console.print(f"  Model: {model_name} | Policy: {policy} | Max rounds: {rounds}")

    result = loop.run(
        eval_command=eval_command if eval_command else None,
        n_candidates_per_round=n_cand,
        n_cognition=cfg.n_cognition,
        verbose=verbose,
    )

    EvolutionLoop.display_summary(result)

    if result.get("best_node"):
        best_path = Path(output_dir or cfg.output_dir) / "best_candidate.py"
        best_path.parent.mkdir(parents=True, exist_ok=True)
        best = result["best_node"]
        best_path.write_text(
            f"# ASI-Evolve Best Candidate (score: {result['best_score']:.4f})\n"
            f"# Motivation: {best.get('motivation', 'N/A')}\n\n"
            f"{best.get('program', '# No program')}",
            encoding="utf-8",
        )
        console.print(f"[green]Meilleur candidat sauvegardé : {best_path}[/green]")


@app.command()
def cognition_add(
    source: str = typer.Argument(..., help="Fichier texte contenant des connaissances"),
    cognition_path: str = typer.Option("./cognition_store", "--cognition", help="Store de cognition"),
    entry_type: str = typer.Option("heuristic", "--type", help="Type: heuristic|paper_insight|pitfall|design_principle"),
) -> None:
    """Ajoute des connaissances au Cognition Base."""
    from papers_v2.evolution import CognitionBase, CognitionEntry

    cognition = CognitionBase(persist_dir=cognition_path)
    source_path = Path(source)

    if source_path.is_file():
        content = source_path.read_text(encoding="utf-8")
        entry = CognitionEntry(
            content=content,
            source=str(source_path.name),
            entry_type=entry_type,
            tags=[entry_type],
        )
        cognition.add_entry(entry)
        console.print(f"[green]Entrée ajoutée: {entry.id} ({entry_type})[/green]")
    elif source_path.is_dir():
        count = 0
        for txt_file in source_path.glob("**/*.txt"):
            content = txt_file.read_text(encoding="utf-8")
            entry = CognitionEntry(
                content=content,
                source=str(txt_file.name),
                entry_type=entry_type,
                tags=[entry_type],
            )
            cognition.add_entry(entry)
            count += 1
        console.print(f"[green]{count} entrées ajoutées depuis {source}[/green]")
    else:
        entry = CognitionEntry(
            content=source,
            source="cli",
            entry_type=entry_type,
            tags=[entry_type],
        )
        cognition.add_entry(entry)
        console.print(f"[green]Entrée ajoutée: {entry.id}[/green]")


@app.command()
def cognition_list(
    cognition_path: str = typer.Option("./cognition_store", "--cognition", help="Store de cognition"),
    n: int = typer.Option(20, "--limit", "-n", help="Nombre d'entrées"),
) -> None:
    """Liste les entrées du Cognition Base."""
    from papers_v2.evolution import CognitionBase

    cognition = CognitionBase(persist_dir=cognition_path)
    entries = cognition.get_all_entries()

    if not entries:
        console.print("[yellow]Aucune entrée de cognition.[/yellow]")
        return

    table = Table(title=f"Cognition Base ({len(entries)} entrées)")
    table.add_column("ID", style="cyan")
    table.add_column("Type", style="green")
    table.add_column("Source", style="magenta")
    table.add_column("Content", style="white", no_wrap=False)

    for entry in entries[:n]:
        table.add_row(entry.id, entry.entry_type, entry.source, entry.content[:200])

    console.print(table)


@app.command()
def evolution_status(
    db_path: str = typer.Option("./evolution_db.json", "--db", help="Base de données d'évolution"),
) -> None:
    """Affiche le statut de la base de données d'évolution."""
    from papers_v2.evolution import EvolutionDatabase

    db = EvolutionDatabase(persist_path=db_path)
    stats = db.stats()

    table = Table(title="Evolution Database Status")
    table.add_column("Metric", style="cyan")
    table.add_column("Value", style="magenta")

    table.add_row("Total Nodes", str(stats["total_nodes"]))
    table.add_row("Mean Score", f"{stats['mean_score']:.4f}")
    table.add_row("Max Score", f"{stats['max_score']:.4f}")
    table.add_row("Min Score", f"{stats['min_score']:.4f}")
    table.add_row("Std Score", f"{stats['std_score']:.4f}")

    console.print(table)

    if stats["total_nodes"] > 0:
        top = db.top_k(5)
        table2 = Table(title="Top 5 Candidates")
        table2.add_column("ID", style="cyan")
        table2.add_column("Score", style="green")
        table2.add_column("Motivation", style="white", no_wrap=False)

        for node in top:
            table2.add_row(node.id, f"{node.score:.4f}", node.motivation[:150])

        console.print(table2)


# ── Paper Registry Commands ───────────────────────────────────────

@app.command()
def registry_import(
    registry: str = typer.Option("./papers_registry.json", "--registry", "-r", help="Fichier registre JSON"),
    knowledge_graph: str = typer.Option("./knowledge_graph.json", "--kg", help="Chemin du graphe de connaissances"),
    vector_store: str = typer.Option("./vector_store", "--vs", help="Chemin du store vectoriel"),
    cognition_path: str = typer.Option("./cognition_store", "--cognition", help="Store de cognition"),
    targets: str = typer.Option("all", "--targets", help="Cibles: all|cognition|kg|vs"),
) -> None:
    """Importe les papiers du registre dans CognitionBase, KnowledgeGraph et VectorStore."""
    from papers_v2.evolution import CognitionBase
    from papers_v2.knowledge.graph import KnowledgeGraph
    from papers_v2.knowledge.papers_registry import PaperRegistry
    from papers_v2.knowledge.vector_store import PaperVectorStore

    reg = PaperRegistry(
        registry_path=registry,
        knowledge_graph_path=knowledge_graph,
        vector_store_path=vector_store,
        cognition_path=cognition_path,
    )
    console.print(f"[bold]Paper Registry: {len(reg.papers)} papers[/bold]")

    if targets in ("all", "cognition"):
        ct = CognitionBase(persist_dir=cognition_path)
        n = reg.import_to_cognition_base(ct)
        console.print(f"[green]CognitionBase: {n} entrees[/green]")

    if targets in ("all", "kg"):
        kg = KnowledgeGraph(path=knowledge_graph)
        n = reg.import_to_knowledge_graph(kg)
        console.print(f"[green]KnowledgeGraph: {n} noeuds papier[/green]")

    if targets in ("all", "vs"):
        vs = PaperVectorStore(persist_dir=vector_store)
        n = reg.import_to_vector_store(vs)
        console.print(f"[green]VectorStore: {n} documents[/green]")

    console.print(f"[bold green]Import termine.[/bold green]")


@app.command()
def registry_list(
    registry: str = typer.Option("./papers_registry.json", "--registry", "-r", help="Fichier registre JSON"),
    tag: str = typer.Option("", "--tag", help="Filtrer par tag"),
    domain: str = typer.Option("", "--domain", help="Filtrer par domaine"),
    query: str = typer.Option("", "--query", "-q", help="Recherche textuelle"),
    limit: int = typer.Option(50, "--limit", "-n", help="Nombre de resultats"),
) -> None:
    """Liste les papiers du registre avec filtres."""
    from papers_v2.knowledge.papers_registry import PaperRegistry

    reg = PaperRegistry(registry_path=registry)

    if tag:
        papers = reg.filter_by_tag(tag)
    elif domain:
        papers = reg.filter_by_domain(domain)
    elif query:
        papers = reg.search(query)
    else:
        papers = reg.papers

    reg.display_table(papers, title=f"Paper Registry ({len(papers)} papers)", limit=limit)


@app.command()
def registry_stats(
    registry: str = typer.Option("./papers_registry.json", "--registry", "-r", help="Fichier registre JSON"),
) -> None:
    """Affiche les statistiques du registre de papiers."""
    from papers_v2.knowledge.papers_registry import PaperRegistry

    reg = PaperRegistry(registry_path=registry)
    stats = reg.stats()

    table = Table(title="Paper Registry Statistics")
    table.add_column("Metric", style="cyan")
    table.add_column("Value", style="magenta")

    table.add_row("Total Papers", str(stats["total_papers"]))
    table.add_row("Total Tags", str(stats["total_tags"]))
    table.add_row("Years", str(dict(sorted(stats["by_year"].items()))))
    table.add_row("Domains", str(dict(sorted(stats["by_domain"].items()))))

    console.print(table)

    console.print("\n[bold]Tags disponibles:[/bold]")
    for tag in reg.list_tags()[:20]:
        console.print(f"  {tag}")


@app.command()
def registry_show(
    paper_id: str = typer.Argument(..., help="ID du papier"),
    registry: str = typer.Option("./papers_registry.json", "--registry", "-r", help="Fichier registre JSON"),
) -> None:
    """Affiche les details d'un papier du registre."""
    from papers_v2.knowledge.papers_registry import PaperRegistry

    reg = PaperRegistry(registry_path=registry)
    paper = reg.get_paper(paper_id)

    if not paper:
        console.print(f"[red]Papier '{paper_id}' non trouve.[/red]")
        raise typer.Exit(code=1)

    console.print(f"\n[bold magenta]{paper['title']}[/bold magenta]")
    console.print(f"[cyan]ID:[/cyan] {paper['id']}")
    console.print(f"[cyan]Authors:[/cyan] {', '.join(paper.get('authors', []))}")
    console.print(f"[cyan]Year:[/cyan] {paper.get('year', 'N/A')}")
    console.print(f"[cyan]Source:[/cyan] {paper.get('source', 'N/A')}")
    console.print(f"[cyan]URL:[/cyan] {paper.get('url', 'N/A')}")
    console.print(f"[cyan]Domain:[/cyan] {paper.get('domain', 'N/A')}")
    console.print(f"[cyan]Relevance:[/cyan] {paper.get('relevance_score', 0):.2f}")
    console.print(f"[cyan]Tags:[/cyan] {', '.join(paper.get('tags', []))}")
    console.print(f"\n[bold]Abstract:[/bold]\n{paper.get('abstract', 'N/A')}")
    console.print(f"\n[bold green]Key Insight:[/bold green]\n{paper.get('key_insight', 'N/A')}")


def _display_summary(report: AnalysisReport) -> None:
    table = Table(title="Résumé d'analyse PAPERS V2")
    table.add_column("Métrique", style="cyan")
    table.add_column("Valeur", style="magenta")

    table.add_row("ID", report.publication.id)
    table.add_row("Titre", report.publication.title)
    table.add_row("Source", report.publication.source or "N/A")
    table.add_row("Score de reproductibilité", f"{report.reproducibility.score:.2f}")
    table.add_row("Score d'intégration", f"{report.integration.score:.2f}")
    table.add_row("Recommandation", report.recommendation.value)
    table.add_row("Interprétation", report.integration.interpretation)

    console.print(table)


def main() -> None:
    app()


if __name__ == "__main__":
    main()
