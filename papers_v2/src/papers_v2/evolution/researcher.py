from __future__ import annotations

import json
from typing import Any

from loguru import logger

from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode
from papers_v2.llm.client import OllamaClient


EVOLUTION_RESEARCHER_SYSTEM = (
    "You are an AI research scientist specialized in designing and improving algorithms, "
    "architectures, and data pipelines. You receive context from prior experiments and "
    "domain knowledge, and must produce a complete, runnable program along with a clear "
    "motivation explaining your design choices."
)

RESEARCHER_PROMPT = (
    "## Task Description\n"
    "{task_description}\n\n"
    "## Retrieved Domain Knowledge (Cognition)\n"
    "{cognition_items}\n\n"
    "## Prior Experiment Context\n"
    "{context_nodes}\n\n"
    "## Instructions\n"
    "Based on the task description, domain knowledge, and prior experimental results above, "
    "design the next candidate program. Produce your response as a JSON object with:\n"
    "- \"motivation\": A detailed natural-language explanation of your design choices, why "
    "you think this approach will improve over prior attempts, and what hypotheses you are testing.\n"
    "- \"program\": The complete, runnable code for the candidate solution.\n"
    "- \"diff_summary\": (optional) If modifying an existing program, describe the changes made.\n\n"
    "IMPORTANT: The program must be complete, self-contained, and immediately executable.\n"
)

DIFF_RESEARCHER_PROMPT = (
    "## Task Description\n"
    "{task_description}\n\n"
    "## Parent Program (to modify)\n"
    "```\n{parent_program}\n```\n\n"
    "## Retrieved Domain Knowledge (Cognition)\n"
    "{cognition_items}\n\n"
    "## Prior Experiment Context\n"
    "{context_nodes}\n\n"
    "## Instructions\n"
    "You are modifying the parent program above. Produce localized, targeted changes. "
    "Respond as a JSON object with:\n"
    "- \"motivation\": Explanation of what you are changing and why.\n"
    "- \"program\": The COMPLETE modified program (not just the diff).\n"
    "- \"diff_summary\": Concise description of the changes made.\n"
)


class Researcher:
    def __init__(
        self,
        model: str = "gemma4:e2b",
        cognition: CognitionBase | None = None,
        database: EvolutionDatabase | None = None,
        use_llm: bool = True,
    ) -> None:
        self.model = model
        self.use_llm = use_llm
        self.client = OllamaClient(model=model) if use_llm else None
        self.cognition = cognition
        self.database = database

    def generate(
        self,
        task_description: str,
        context_nodes: list[EvolutionNode] | None = None,
        cognition_entries: list[CognitionEntry] | None = None,
        cognition_query: str = "",
        parent_program: str = "",
        use_diff: bool = False,
        n_cognition: int = 5,
        max_tokens: int = 4096,
    ) -> dict[str, Any]:
        if cognition_entries is None and self.cognition and cognition_query:
            cognition_entries = self.cognition.retrieve(cognition_query, n_results=n_cognition)

        cognition_text = self._format_cognition(cognition_entries or [])
        context_text = self._format_context(context_nodes or [])

        if use_diff and parent_program:
            prompt = DIFF_RESEARCHER_PROMPT.format(
                task_description=task_description,
                parent_program=parent_program[:8000],
                cognition_items=cognition_text,
                context_nodes=context_text,
            )
        else:
            prompt = RESEARCHER_PROMPT.format(
                task_description=task_description,
                cognition_items=cognition_text,
                context_nodes=context_text,
            )

        if not self.use_llm or not self.client or not self.client.is_available():
            logger.warning("Ollama indisponible pour le Researcher.")
            return self._fallback_generation(task_description)

        try:
            result = self.client.generate_json(
                prompt,
                system=EVOLUTION_RESEARCHER_SYSTEM,
                max_tokens=max_tokens,
            )
            return {
                "motivation": result.get("motivation", ""),
                "program": result.get("program", ""),
                "diff_summary": result.get("diff_summary", ""),
            }
        except Exception as e:
            logger.warning(f"Erreur LLM dans Researcher: {e}")
            return self._fallback_generation(task_description)

    def _format_cognition(self, entries: list[CognitionEntry]) -> str:
        if not entries:
            return "(Aucune connaissance de domaine disponible)"
        lines = []
        for i, entry in enumerate(entries, 1):
            lines.append(f"### Cognition {i} (source: {entry.source}, type: {entry.entry_type})")
            lines.append(entry.content[:1500])
            lines.append("")
        return "\n".join(lines)

    def _format_context(self, nodes: list[EvolutionNode]) -> str:
        if not nodes:
            return "(Aucun contexte experimental disponible - premier essai)"
        lines = []
        for i, node in enumerate(nodes, 1):
            score_str = f"Score: {node.score:.4f}" if node.score else "Score: N/A"
            lines.append(f"### Experiment {i} ({node.id}) - {score_str}")
            lines.append(f"Motivation: {node.motivation[:300]}")
            if node.analysis:
                lines.append(f"Analysis: {node.analysis[:300]}")
            if node.results:
                lines.append(f"Results: {json.dumps(node.results, ensure_ascii=False)[:500]}")
            lines.append("")
        return "\n".join(lines)

    def _fallback_generation(self, task_description: str) -> dict[str, Any]:
        return {
            "motivation": "Generation fallback: LLM indisponible. Utilisation de l'heuristique par defaut.",
            "program": f"# Fallback program for task: {task_description[:100]}\n# LLM unavailable, manual implementation required.\ndef main():\n    pass\n",
            "diff_summary": "Fallback: no changes made.",
        }

    def generate_batch(
        self,
        task_description: str,
        n_candidates: int = 3,
        cognition_query: str = "",
        database: EvolutionDatabase | None = None,
        n_cognition: int = 5,
    ) -> list[dict[str, Any]]:
        db = database or self.database
        candidates = []
        parent = None
        if db and db.count() > 0:
            parent = db.best()

        for i in range(n_candidates):
            context_nodes = db.sample(n=5, policy="ucb1") if db else []
            candidate = self.generate(
                task_description=task_description,
                context_nodes=context_nodes,
                cognition_query=cognition_query,
                parent_program=parent.program if parent and i > 0 else "",
                use_diff=bool(parent and i > 0),
                n_cognition=n_cognition,
            )
            candidates.append(candidate)
        return candidates
