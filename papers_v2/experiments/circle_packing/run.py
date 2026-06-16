#!/usr/bin/env python3
"""Circle packing benchmark runner for PAPERS V2."""

import sys, os, json, math, time, tempfile
from pathlib import Path

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../.."))

from papers_v2.evolution.cognition import CognitionBase
from papers_v2.evolution.database import EvolutionDatabase, SamplingPolicy
from papers_v2.evolution.loop import EvolutionLoop

EVAL_PATH = os.path.join(os.path.dirname(__file__), "evaluator.py")
COG_PATH = os.path.join(os.path.dirname(__file__), "cognition_data")

def evaluate_circle_packing(program: str) -> dict:
    """Write program to temp file, run evaluator, parse JSON output."""
    import subprocess
    with tempfile.NamedTemporaryFile(mode="w", suffix=".py", delete=False) as f:
        f.write(program)
        tmp_path = f.name

    try:
        result = subprocess.run(
            ["python3", EVAL_PATH, tmp_path],
            capture_output=True, text=True, timeout=30,
        )
        output = result.stdout.strip()
        if output:
            return json.loads(output)
        return {"success": False, "score": 0.0, "error": result.stderr[:200]}
    except subprocess.TimeoutExpired:
        return {"success": False, "score": 0.0, "error": "timeout"}
    except Exception as e:
        return {"success": False, "score": 0.0, "error": str(e)}
    finally:
        try:
            os.unlink(tmp_path)
        except Exception:
            pass


def main():
    N_ROUNDS = 20
    N_CANDIDATES = 5

    print(f"Circle Packing Benchmark - {N_ROUNDS} rounds, {N_CANDIDATES} candidates/round")
    print("=" * 60)

    # Baseline scores
    import subprocess
    baseline = subprocess.run(
        ["python3", EVAL_PATH], capture_output=True, text=True
    )
    print(f"Baselines: {baseline.stdout.strip()}")

    cognition = CognitionBase(persist_dir=COG_PATH)
    db = EvolutionDatabase(persist_path="./experiments/circle_packing/evolution_db.json")

    loop = EvolutionLoop(
        task_description=(
            "Pack 26 circles in a unit square to maximize the minimum radius. "
            "Implement function place_circles(n) returning list of (x, y, r) tuples. "
            "Use hexagonal packing, greedy placement, force-directed layout, or iterative refinement."
        ),
        cognition=cognition,
        database=db,
        model="gemma4:e2b",
        sampling_policy=SamplingPolicy.UCB1,
        max_rounds=N_ROUNDS,
        patience=8,
        use_llm=False,  # fast mode for benchmark
        db_path="./experiments/circle_packing/evolution_db.json",
        cognition_path=COG_PATH,
        output_dir="./experiments/circle_packing/output",
    )

    start = time.time()
    result = loop.run(
        eval_function=evaluate_circle_packing,
        n_candidates_per_round=N_CANDIDATES,
        n_cognition=3,
        verbose=True,
    )

    elapsed = time.time() - start
    print(f"\n{'='*60}")
    print(f"Done in {elapsed:.1f}s")
    print(f"Best score: {result['best_score']:.6f}")
    print(f"Rounds: {result['total_rounds']}")
    print(f"Candidates: {result['total_candidates']}")
    print(f"DB stats: {result['database_stats']}")

    if result.get("best_node"):
        best = result["best_node"]
        print(f"\nBest program ({best.get('id', '?')}):")
        print(best.get("program", "")[:500])

    # Save best program
    out = Path("./experiments/circle_packing/output")
    out.mkdir(parents=True, exist_ok=True)
    (out / "best_score.txt").write_text(str(result["best_score"]))
    if result.get("best_node"):
        (out / "best_program.py").write_text(result["best_node"].get("program", ""))

    return result


if __name__ == "__main__":
    main()
