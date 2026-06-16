"""
Templates d'experiments pour ASI-Evolve / PAPERS V2.
Inclut le benchmark circle_packing standard.
"""

from __future__ import annotations

import math
import random
from typing import Any


def circle_packing_evaluator(program: str, n_circles: int = 26) -> dict[str, Any]:
    """
    Evalue un programme de circle packing.
    Le programme doit definir une fonction `place_circles(n)` qui retourne
    une liste de (x, y, r) tuples.
    """
    try:
        local_ns: dict[str, Any] = {}
        exec(program, {"math": math, "random": random}, local_ns)

        if "place_circles" not in local_ns:
            return {"success": False, "score": 0.0, "error": "place_circles() non definie"}

        placements = local_ns["place_circles"](n_circles)
        if len(placements) != n_circles:
            return {"success": False, "score": 0.0, "error": f"Attendu {n_circles} cercles, obtenu {len(placements)}"}

        # Score: sum of areas (minimize unused space is maximize radius sum)
        min_radius = float("inf")
        total_area = 0.0
        for i, (x, y, r) in enumerate(placements):
            if r <= 0:
                return {"success": False, "score": 0.0, "error": f"Cercle {i}: rayon {r} <= 0"}
            if not (0 <= x <= 1 and 0 <= y <= 1):
                return {"success": False, "score": 0.0, "error": f"Cercle {i}: hors limites (x={x}, y={y})"}
            total_area += math.pi * r * r
            min_radius = min(min_radius, r)

            # Check no overlap
            for j in range(i):
                x2, y2, r2 = placements[j]
                dx, dy = x - x2, y - y2
                dist = math.sqrt(dx * dx + dy * dy)
                if dist < r + r2 - 1e-10:
                    return {"success": False, "score": 0.0, "error": f"Chevauchement cercles {i} et {j}"}

        # Score = total area + min radius bonus
        score = total_area + min_radius

        return {
            "success": True,
            "score": score,
            "metrics": {
                "total_area": total_area,
                "min_radius": min_radius,
                "n_circles": n_circles,
                "density": total_area,
            },
        }
    except Exception as e:
        return {"success": False, "score": 0.0, "error": str(e)}


LINEAR_ATTENTION_EVALUATOR_TEMPLATE = """
import importlib.util, sys, json, time

def evaluate(program_path):
    spec = importlib.util.spec_from_file_location("candidate", program_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)

    # Le module doit definir forward(), create_model(), etc.
    model = mod.create_model()
    result = run_benchmark(model)  # Votre benchmark interne

    return json.dumps({"score": result["avg_accuracy"], "metrics": result})

if __name__ == "__main__":
    result = evaluate(sys.argv[1])
    print(result)
"""


def create_experiment_template(experiment_dir: str, task_description: str, initial_program: str = "") -> dict[str, str]:
    """Cree le squelette d'un nouveau repertoire d'experiment."""
    import os
    os.makedirs(experiment_dir, exist_ok=True)

    files: dict[str, str] = {}

    files["input.md"] = f"# Experiment\n\n{task_description}\n"
    files["config.yaml"] = "max_rounds: 50\nsampling_policy: greedy\nn_candidates_per_round: 3\n"
    files["initial_program.py"] = initial_program or "# Baseline program\n\ndef solve():\n    pass\n"
    files["eval.sh"] = "#!/bin/bash\npython evaluator.py \"$1\"\n"

    for name, content in files.items():
        path = os.path.join(experiment_dir, name)
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        if name.endswith(".sh"):
            os.chmod(path, 0o755)

    return files
