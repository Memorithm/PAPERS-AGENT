"""
Research Frenzy - Autonomous paper-driven code generation engine.
Connects paper discovery → analysis → implementation in a self-reinforcing loop.

Based on ASI-Evolve + all 34 intelligence papers.
"""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

from loguru import logger
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.table import Table

from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode, SamplingPolicy
from papers_v2.evolution.engineer import Engineer
from papers_v2.evolution.researcher import Researcher
from papers_v2.intelligence.falsification import FalsificationEngine
from papers_v2.intelligence.graph_mining import GraphPatternMiner
from papers_v2.intelligence.pattern_induction import PatternInductionEngine
from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
from papers_v2.intelligence.symbolic_reasoning import SymbolicReasoningEngine
from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
from papers_v2.knowledge.graph import KnowledgeGraph
from papers_v2.knowledge.papers_registry import PaperRegistry

console = Console()


class ResearchFrenzy:
    def __init__(
        self,
        task_description: str,
        model: str = "gemma4:e2b",
        max_rounds: int = 10,
        use_llm: bool = True,
        registry_path: str = "./papers_registry.json",
        cognition_path: str = "./cognition_store",
        db_path: str = "./evolution_db.json",
        kg_path: str = "./knowledge_graph.json",
        output_dir: str = "./frenzy_output",
    ) -> None:
        self.task_description = task_description
        self.model = model
        self.max_rounds = max_rounds
        self.use_llm = use_llm
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

        self.registry = PaperRegistry(registry_path=registry_path)
        self.cognition = CognitionBase(persist_dir=cognition_path)
        self.database = EvolutionDatabase(persist_path=db_path)
        self.kg = KnowledgeGraph(path=kg_path)

        self.researcher = Researcher(
            model=model, cognition=self.cognition, database=self.database, use_llm=use_llm,
        )
        self.engineer = Engineer()

        self.pattern_engine = PatternInductionEngine()
        self.falsification = FalsificationEngine()
        self.verifier = CounterexampleGuidedVerifier()
        self.symbolic = SymbolicReasoningEngine()
        self.probabilistic = ProbabilisticReasoner()
        self.graph_miner = GraphPatternMiner()

    def run(
        self,
        eval_function: Any | None = None,
        n_parallel_candidates: int = 3,
        verbose: bool = True,
    ) -> dict[str, Any]:
        start_time = time.time()
        frenzy_log: list[dict[str, Any]] = []

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console,
        ) as progress:
            task = progress.add_task("[cyan]Research Frenzy...", total=None)

            for round_num in range(1, self.max_rounds + 1):
                progress.update(task, description=f"[cyan]Round {round_num}/{self.max_rounds} - Searching papers...")

                relevant_papers = self._search_relevant_papers()
                if verbose:
                    console.print(f"\n[bold]Round {round_num}: {len(relevant_papers)} papers relevantes[/bold]")

                progress.update(task, description=f"[cyan]Round {round_num} - Inducing patterns...")
                induced_patterns = self._induce_patterns()

                progress.update(task, description=f"[cyan]Round {round_num} - Mining graph...")
                graph_patterns = self._mine_graph()

                progress.update(task, description=f"[cyan]Round {round_num} - Generating candidates...")
                candidates = self.researcher.generate_batch(
                    task_description=self.task_description,
                    n_candidates=n_parallel_candidates,
                    cognition_query=self.task_description,
                    database=self.database,
                )

                for cand_idx, candidate in enumerate(candidates):
                    if not candidate.get("program"):
                        continue

                    cid = f"frenzy_r{round_num}_c{cand_idx}"

                    progress.update(task, description=f"[cyan]Round {round_num} - Verifying candidate {cand_idx+1}...")
                    constraints = self._build_constraints(relevant_papers, induced_patterns)
                    result = self.engineer.execute(
                        program=candidate["program"],
                        eval_function=eval_function,
                    )

                    verification = self.verifier.verify(
                        cid, candidate["program"], result, constraints,
                    )

                    progress.update(task, description=f"[cyan]Round {round_num} - Falsifying candidate {cand_idx+1}...")
                    self.falsification.generate_tests(
                        cid, candidate.get("motivation", ""), candidate["program"],
                    )

                    if eval_function:
                        falsifiability = self.falsification.compute_falsifiability_score(cid)
                    else:
                        falsifiability = 0.5

                    progress.update(task, description=f"[cyan]Round {round_num} - Bayesian update...")
                    score = result.get("score", 0.0)
                    self.probabilistic.observe(cid, max(0.01, score))

                    hack_check = self.falsification.detect_reward_hacking(
                        cid, score, falsifiability,
                    )

                    node = EvolutionNode(
                        program=candidate["program"],
                        motivation=candidate.get("motivation", ""),
                        results=result,
                        analysis=json.dumps({
                            "verification": verification.to_dict(),
                            "falsifiability": falsifiability,
                            "reward_hacking": hack_check,
                        }, ensure_ascii=False),
                        score=score,
                        metadata={
                            "round": round_num,
                            "candidate_index": cand_idx,
                            "papers_used": [p.get("id", "") for p in relevant_papers[:5]],
                            "patterns_used": len(induced_patterns),
                        },
                    )
                    self.database.add_node(node)

                    frenzy_log.append({
                        "round": round_num,
                        "candidate": cid,
                        "score": score,
                        "verification_passed": verification.passed,
                        "falsifiability": falsifiability,
                        "reward_hacking_risk": hack_check["risk"],
                    })

                    if verbose:
                        icon = "[green]PASS" if verification.passed else "[red]FAIL"
                        console.print(
                            f"  {icon}[/] c{cand_idx}: score={score:.3f} "
                            f"falsif={falsifiability:.2f} hack={hack_check['risk']}"
                        )

                self.database.save()
                self.pattern_engine.save(str(self.output_dir / "patterns.json"))

                if verbose and round_num % 3 == 0:
                    self._display_interim(frenzy_log)

        total_time = time.time() - start_time
        progress.update(task, completed=True, description="[green]Frenzy complete!")

        return {
            "total_rounds": self.max_rounds,
            "total_candidates": len(frenzy_log),
            "total_time": total_time,
            "best_score": max((e["score"] for e in frenzy_log), default=0),
            "database_stats": self.database.stats(),
            "calibration": self.probabilistic.calibration_report(),
            "top_hypothesis": (
                self.probabilistic.best_hypothesis().to_dict()
                if self.probabilistic.best_hypothesis() else None
            ),
            "graph_report": self.graph_miner.export_report(),
            "log": frenzy_log,
        }

    def _search_relevant_papers(self) -> list[dict[str, Any]]:
        results = self.registry.search(self.task_description)
        if not results:
            tags = self.task_description.lower().split()
            for tag in tags:
                filtered = self.registry.filter_by_tag(tag)
                results.extend(filtered)
        return results[:10]

    def _induce_patterns(self) -> list[Any]:
        history = [
            {"score": n.score, "program": n.program, "motivation": n.motivation,
             "node_id": n.id, "parent_id": n.metadata.get("parent_id", "")}
            for n in self.database.top_k(20)
        ]
        patterns = self.pattern_engine.induce_from_history(history)
        for p in patterns:
            self.pattern_engine.register(p)
        return self.pattern_engine.top_patterns(10)

    def _mine_graph(self) -> list[Any]:
        self.graph_miner.set_graph(self.kg.graph)
        return self.graph_miner.mine_patterns()

    def _build_constraints(
        self,
        papers: list[dict[str, Any]],
        patterns: list[Any],
    ) -> list[dict[str, Any]]:
        constraints: list[dict[str, Any]] = []

        for paper in papers[:5]:
            tags = paper.get("tags", [])
            if "efficiency" in tags or "optimization" in tags:
                constraints.append({
                    "type": "max_complexity",
                    "value": 30,
                })

        if patterns:
            top_sig = patterns[0].signature if hasattr(patterns[0], "signature") else ""
            constraints.append({
                "type": "target_pattern",
                "target": top_sig[:80],
            })

        return constraints

    def _display_interim(self, log: list[dict[str, Any]]) -> None:
        table = Table(title="Frenzy Interim Report")
        table.add_column("Metric", style="cyan")
        table.add_column("Value", style="magenta")

        best = max(log, key=lambda x: x["score"]) if log else None
        table.add_row("Candidates", str(len(log)))
        table.add_row("Best Score", f"{best['score']:.3f}" if best else "N/A")
        table.add_row("Pass Rate", (
            f"{sum(1 for e in log if e['verification_passed']) / len(log):.1%}"
            if log else "0%"
        ))
        table.add_row("High Hack Risk", str(
            sum(1 for e in log if e['reward_hacking_risk'] == 'HIGH')
        ))
        table.add_row("Patterns Induced", str(len(self.pattern_engine.patterns)))
        table.add_row("Graph Patterns", str(len(self.graph_miner.patterns)))

        console.print(table)
