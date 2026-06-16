from __future__ import annotations

import json
from typing import Any

from loguru import logger

from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode
from papers_v2.llm.client import OllamaClient


EVOLUTION_RESEARCHER_SYSTEM = (
    "You are an expert Python programmer. Your ONLY job is to write complete, "
    "runnable Python code that solves the given task. Always respond with a JSON "
    "object containing a 'program' field with working code and a 'motivation' field "
    "explaining your approach. The code MUST work immediately when executed with python3."
)

RESEARCHER_PROMPT = (
    "## Task\n{task_description}\n\n"
    "## Knowledge\n{cognition_items}\n\n"
    "## Prior Results\n{context_nodes}\n\n"
    "## Requirements\n"
    "1. Write a SINGLE Python function that solves the task\n"
    "2. The code must be COMPLETE (all imports, all helper functions, all logic)\n"
    "3. The code must be self-contained (no external files needed)\n"
    "4. Use only the Python standard library (math, random, itertools OK)\n"
    "5. Return ONLY valid JSON: {{\"program\": \"...\", \"motivation\": \"...\"}}\n"
    "6. The program field must contain the FULL source code as a string\n"
)
PROMPT_TAIL = (
    "\n\nRespond with JSON:\n"
    "{{\"motivation\": \"explain your approach in 1-2 sentences\", "
    "\"program\": \"import math\\\\n\\\\ndef solve(input):\\\\n    return result\\\\n\"}}"
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
        prompt += PROMPT_TAIL

        if not self.use_llm or not self.client or not self.client.is_available():
            logger.warning("Ollama indisponible pour le Researcher.")
            return self._fallback_generation(task_description)

        try:
            result = self.client.generate_json(
                prompt,
                system=EVOLUTION_RESEARCHER_SYSTEM,
                max_tokens=max_tokens,
            )
            program = result.get("program", "")
            # Validate: must contain a function definition
            if program and ("def " in program or "import " in program):
                return {
                    "motivation": result.get("motivation", ""),
                    "program": self._clean_program(program),
                    "diff_summary": result.get("diff_summary", ""),
                }
            else:
                logger.warning("Generated program is invalid, using fallback")
                return self._fallback_generation(task_description)
        except Exception as e:
            logger.warning(f"Erreur LLM dans Researcher: {e}")
            return self._fallback_generation(task_description)

    def _clean_program(self, program: str) -> str:
        program = program.strip()
        if program.startswith("```"):
            lines = program.split("\n")
            if lines[0].startswith("```"):
                lines = lines[1:]
            if lines and lines[-1].strip().startswith("```"):
                lines = lines[:-1]
            program = "\n".join(lines).strip()
        if (program.startswith('"') and program.endswith('"')) or \
           (program.startswith("'") and program.endswith("'")):
            program = program[1:-1]
        program = program.replace("\\n", "\n").replace("\\t", "\t")
        # Cap iterations for performance: max 5000 total loops
        import re
        program = re.sub(r'steps_per_temp\s*=\s*\d+', 'steps_per_temp = 20', program)
        program = re.sub(r'(?<!_)iterations?\s*=\s*\d{4,}', 'iterations = 200', program)
        program = re.sub(r'T_start\s*=\s*\d+\.\d+', 'T_start = 5.0', program)
        return program

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
        task_lower = task_description.lower()

        templates = []

        if "circle" in task_lower or "pack" in task_lower:
            templates = [
                ("Hexagonal grid packing baseline", '''import math, random

def place_circles(n):
    """Hexagonal packing of n circles in unit square."""
    cols = int(math.ceil(math.sqrt(n * 2 / math.sqrt(3))))
    r = 1.0 / (2 * cols)
    row_h = r * math.sqrt(3)
    result = []
    placed = 0
    row = 0
    while placed < n:
        n_cols = cols if row % 2 == 0 else cols - 1
        for col in range(n_cols):
            if placed >= n:
                break
            x = r * (1 + 2 * col) if row % 2 == 0 else r * (2 + 2 * col)
            y = r + row * row_h
            if y + r <= 1.0 and x - r >= 0 and x + r <= 1.0:
                result.append((x, y, r))
                placed += 1
        row += 1
    return result[:n]'''),
                ("Greedy placement with random search", '''import math, random

def place_circles(n, trials=500):
    """Greedy circle placement maximizing min distance."""
    best = None
    best_r = 0.0
    for _ in range(trials):
        r = 0.5 / math.sqrt(n)
        placed = []
        for _ in range(n):
            candidates = []
            for _ in range(50):
                x = random.uniform(r, 1 - r)
                y = random.uniform(r, 1 - r)
                min_dist = min(
                    [min(x, 1-x, y, 1-y)] +
                    [math.sqrt((x-px)**2 + (y-py)**2) - pr for px, py, pr in placed]
                )
                candidates.append((min_dist, x, y))
            if not candidates:
                break
            _, best_x, best_y = max(candidates)
            placed.append((best_x, best_y, r))
        if len(placed) == n:
            actual_r = min(
                min(min(p[0], 1-p[0], p[1], 1-p[1]) for p in placed),
                min(min(math.sqrt((p1[0]-p2[0])**2+(p1[1]-p2[1])**2)/2 for p2 in placed[i+1:]) for i, p1 in enumerate(placed))
            )
            if actual_r > best_r:
                best_r = actual_r
                best = [(x, y, actual_r) for x, y, _ in placed]
    return best if best else [(0.5, 0.5, 0.01)] * n'''),
                ("Simulated annealing", '''import math, random

def place_circles(n, iterations=5000):
    """Simulated annealing for circle packing."""
    r = 0.5 / math.sqrt(n)
    config = [(random.uniform(r, 1-r), random.uniform(r, 1-r)) for _ in range(n)]

    def compute_r(positions):
        min_d = float('inf')
        for i, (x1, y1) in enumerate(positions):
            min_d = min(min_d, x1, 1-x1, y1, 1-y1)
            for j in range(i+1, n):
                d = math.sqrt((x1-positions[j][0])**2 + (y1-positions[j][1])**2)
                min_d = min(min_d, d)
        return min_d / 2.0

    best_r = compute_r(config)
    best_config = list(config)
    temp = 0.1

    for it in range(iterations):
        i = random.randrange(n)
        old_pos = config[i]
        config[i] = (random.uniform(r, 1-r), random.uniform(r, 1-r))
        new_r = compute_r(config)

        if new_r > best_r or random.random() < math.exp((new_r - best_r) / max(temp, 1e-10)):
            best_r = new_r
            best_config = list(config)
        else:
            config[i] = old_pos
        temp *= 0.999

    return [(x, y, best_r) for x, y in best_config]'''),
            ]
        elif "sort" in task_lower or "optimize" in task_lower:
            templates = [
                ("Optimization baseline", '''def optimize(data):
    """Sort and return optimized data."""
    return sorted(data)'''),
                ("Greedy optimization", '''import random
def optimize(data):
    """Greedy optimization with random restarts."""
    best = sorted(data)
    for _ in range(100):
        candidate = list(data)
        random.shuffle(candidate)
        if sum(candidate) < sum(best):
            best = candidate
    return best'''),
            ]
        elif "train" in task_lower or "neural" in task_lower:
            templates = [
                ("Neural network baseline", '''import math
def train_model(data, epochs=10):
    """Simple neural network training loop."""
    weights = [0.0] * len(data[0]) if data else []
    for _ in range(epochs):
        for sample in data:
            pred = sum(w * x for w, x in zip(weights, sample))
            error = pred - 1.0
            weights = [w - 0.01 * error * x for w, x in zip(weights, sample)]
    return weights'''),
            ]
        else:
            templates = [
                ("Generic solver v1", f"# Solution for: {task_description[:80]}\ndef solve(data):\n    return data\n"),
                ("Generic solver v2", f"# Solution for: {task_description[:80]}\nimport math\ndef solve(data):\n    result = []\n    for item in data:\n        result.append(item * 2)\n    return result\n"),
                ("Generic solver v3", f"# Solution for: {task_description[:80]}\ndef solve(data):\n    best = data[0] if data else None\n    for item in data:\n        if item < best:\n            best = item\n    return best\n"),
            ]

        import random as _random
        motivation, program = _random.choice(templates)
        return {
            "motivation": motivation,
            "program": program,
            "diff_summary": f"Heuristic fallback: {motivation}",
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
