from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any, Callable

from loguru import logger
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TaskProgressColumn
from rich.table import Table

from papers_v2.evolution.analyzer import EvolutionAnalyzer
from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode, SamplingPolicy
from papers_v2.evolution.engineer import Engineer
from papers_v2.evolution.researcher import Researcher

console = Console()


class EvolutionLoop:
    def __init__(
        self,
        task_description: str,
        cognition: CognitionBase | None = None,
        database: EvolutionDatabase | None = None,
        researcher: Researcher | None = None,
        engineer: Engineer | None = None,
        analyzer: EvolutionAnalyzer | None = None,
        model: str = "gemma4:e2b",
        sampling_policy: SamplingPolicy = SamplingPolicy.GREEDY,
        max_rounds: int = 50,
        target_score: float | None = None,
        patience: int = 10,
        use_llm: bool = True,
        db_path: str = "./evolution_db.json",
        cognition_path: str = "./cognition_store",
        output_dir: str = "./evolution_output",
    ) -> None:
        self.task_description = task_description
        self.model = model
        self.sampling_policy = sampling_policy
        self.max_rounds = max_rounds
        self.target_score = target_score
        self.patience = patience
        self.use_llm = use_llm
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

        self.cognition = cognition or CognitionBase(persist_dir=cognition_path)
        self.database = database or EvolutionDatabase(persist_path=db_path)
        self.researcher = researcher or Researcher(
            model=model, cognition=self.cognition, database=self.database, use_llm=use_llm
        )
        self.engineer = engineer or Engineer()
        self.analyzer = analyzer or EvolutionAnalyzer(model=model, use_llm=use_llm)

        self.history: list[dict[str, Any]] = []
        self.best_node: EvolutionNode | None = None
        self.best_score: float = 0.0
        self.rounds_without_improvement: int = 0

    def run(
        self,
        eval_function: Callable[[str], dict[str, Any]] | None = None,
        eval_command: str | None = None,
        n_candidates_per_round: int = 3,
        n_context_nodes: int = 5,
        n_cognition: int = 5,
        prompt_cognition_query: str = "",
        verbose: bool = True,
    ) -> dict[str, Any]:
        if not eval_function and not eval_command:
            raise ValueError("Either eval_function or eval_command must be provided")

        start_time = time.time()
        stopped_early = False

        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            TaskProgressColumn(),
            console=console,
        ) as progress:
            task = progress.add_task("[cyan]Evolution...", total=self.max_rounds)

            for round_num in range(1, self.max_rounds + 1):
                if verbose:
                    console.print(f"\n[bold cyan]=== Round {round_num}/{self.max_rounds} ===[/bold cyan]")

                round_start = time.time()

                # Sample context from database
                context_nodes = self.database.sample(
                    n=n_context_nodes,
                    policy=self.sampling_policy,
                )

                # Retrieve cognition items
                cognition_query = (
                    prompt_cognition_query
                    or (context_nodes[0].analysis if context_nodes else "")
                    or self.task_description
                )

                # Generate candidates
                candidates = self.researcher.generate_batch(
                    task_description=self.task_description,
                    n_candidates=n_candidates_per_round,
                    cognition_query=cognition_query,
                    database=self.database,
                    n_cognition=n_cognition,
                )

                # Execute and analyze candidates
                round_best_score = -float("inf")
                round_best_node = None

                for cand_idx, candidate in enumerate(candidates):
                    if not candidate.get("program"):
                        continue

                    # Execute
                    result = self.engineer.execute(
                        program=candidate["program"],
                        eval_command=eval_command,
                        eval_function=eval_function,
                    )

                    # Compute fitness - score can be at top level or in metrics
                    raw_score = result.get("score", result.get("metrics", {}).get("score", 0.0))
                    fitness = float(raw_score)
                    result["score"] = fitness

                    # Analyze result
                    analysis = self.analyzer.analyze(
                        motivation=candidate.get("motivation", ""),
                        program=candidate.get("program", ""),
                        results=result,
                    )

                    # Create evolution node
                    node = EvolutionNode(
                        program=candidate["program"],
                        motivation=candidate.get("motivation", ""),
                        results=result,
                        analysis=json.dumps(analysis, ensure_ascii=False),
                        score=fitness,
                        metadata={
                            "round": round_num,
                            "candidate_index": cand_idx,
                            "runtime_seconds": result.get("runtime_seconds", 0),
                            "diff_summary": candidate.get("diff_summary", ""),
                        },
                    )

                    # Add cognition update if analyzer produced one
                    cognition_update = analysis.get("cognition_update", "")
                    if cognition_update:
                        entry = CognitionEntry(
                            content=cognition_update,
                            source=f"Round {round_num} Candidate {cand_idx + 1}",
                            entry_type="learned_insight",
                            tags=["evolved", f"score_{fitness:.2f}"],
                        )
                        self.cognition.add_entry(entry)

                    # Store in database
                    self.database.add_node(node)
                    self.history.append({
                        "round": round_num,
                        "node_id": node.id,
                        "score": fitness,
                        "motivation": node.motivation[:200],
                    })

                    if fitness > round_best_score:
                        round_best_score = fitness
                        round_best_node = node

                    if verbose:
                        status = "[green]OK[/green]" if result.get("success") else "[red]FAIL[/red]"
                        console.print(
                            f"  Candidate {cand_idx + 1}: score={fitness:.4f} {status} "
                            f"({result.get('runtime_seconds', 0):.1f}s)"
                        )

                # Update best
                if round_best_node and round_best_score > self.best_score:
                    self.best_score = round_best_score
                    self.best_node = round_best_node
                    self.rounds_without_improvement = 0
                    if verbose:
                        console.print(f"  [bold green]New best! Score={self.best_score:.4f}[/bold green]")
                else:
                    self.rounds_without_improvement += 1

                # Check early stopping
                if self.target_score and self.best_score >= self.target_score:
                    if verbose:
                        console.print(f"[bold green]Target score {self.target_score} reached at round {round_num}![/bold green]")
                    stopped_early = True
                    progress.update(task, completed=self.max_rounds)
                    break

                if self.rounds_without_improvement >= self.patience:
                    if verbose:
                        console.print(f"[yellow]No improvement for {self.patience} rounds. Stopping.[/yellow]")
                    stopped_early = True
                    progress.update(task, completed=self.max_rounds)
                    break

                # Prune database periodically
                if round_num % 10 == 0:
                    self.database.prune_bottom(keep_top=50)

                # Save state
                self.database.save()
                self._save_state()

                if verbose:
                    elapsed = time.time() - round_start
                    console.print(f"  Round {round_num} completed in {elapsed:.1f}s")

                progress.update(task, advance=1)

        total_time = time.time() - start_time

        return {
            "success": len(self.history) > 0,
            "best_score": self.best_score,
            "best_node": self.best_node.to_dict() if self.best_node else None,
            "total_rounds": len(set(h["round"] for h in self.history)),
            "total_candidates": len(self.history),
            "database_stats": self.database.stats(),
            "cognition_count": self.cognition.count(),
            "total_time_seconds": total_time,
            "stopped_early": stopped_early,
            "history": self.history,
        }

    def _save_state(self) -> None:
        state = {
            "task_description": self.task_description,
            "model": self.model,
            "sampling_policy": self.sampling_policy.value,
            "best_score": self.best_score,
            "rounds_without_improvement": self.rounds_without_improvement,
            "history": self.history[-20:],
        }
        state_path = self.output_dir / "evolution_state.json"
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2))

    @staticmethod
    def display_summary(result: dict[str, Any]) -> None:
        table = Table(title="ASI-Evolve Summary")
        table.add_column("Metric", style="cyan")
        table.add_column("Value", style="magenta")

        table.add_row("Best Score", f"{result['best_score']:.4f}")
        table.add_row("Total Rounds", str(result["total_rounds"]))
        table.add_row("Total Candidates", str(result["total_candidates"]))
        table.add_row("Total Time", f"{result['total_time_seconds']:.1f}s")
        table.add_row("Stopped Early", str(result["stopped_early"]))
        table.add_row("Cognition Entries", str(result["cognition_count"]))

        stats = result.get("database_stats", {})
        table.add_row("DB Mean Score", f"{stats.get('mean_score', 0):.4f}")
        table.add_row("DB Max Score", f"{stats.get('max_score', 0):.4f}")

        console.print(table)
