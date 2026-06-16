"""Benchmark qwen3:4b vs gemma4:e2b on a single paper with PAPERS V2."""
from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

import typer
from loguru import logger

from papers_v2.core.orchestrator import PapersEngine
from papers_v2.extraction.extractors import ExtractionPipeline


def _analyze(source: str, model: str, output_dir: Path) -> dict[str, Any]:
    logger.info(f"Benchmarking model={model} on {source}")
    pipeline = ExtractionPipeline()
    engine = PapersEngine(use_llm=True, llm_model=model)

    start = time.perf_counter()
    publication = pipeline.extract(source)
    report = engine.analyze(publication, deep=False)
    elapsed = time.perf_counter() - start

    output_dir.mkdir(parents=True, exist_ok=True)
    safe_model = model.replace(":", "_")
    json_path = output_dir / f"report_{safe_model}.json"
    json_path.write_text(report.model_dump_json(indent=2), encoding="utf-8")

    return {
        "model": model,
        "duration_seconds": round(elapsed, 2),
        "integration_score": round(report.integration.score, 4),
        "reproducibility_score": round(report.reproducibility.score, 4),
        "recommendation": report.recommendation.value,
        "contributions_count": len(report.scientific_contributions),
        "equations_count": len(report.mathematical_analysis.equations),
        "risks_count": len(report.risks),
        "json_path": str(json_path),
    }


def main(
    source: str = typer.Argument(..., help="Chemin ou URL du papier à analyser"),
    output_dir: str = typer.Option("./examples/output/benchmark", "--output-dir", help="Répertoire de sortie"),
    models: list[str] = typer.Option(["qwen3:4b", "gemma4:e2b"], "--model", help="Modèles à comparer"),
) -> None:
    out = Path(output_dir)
    results = [_analyze(source, model, out) for model in models]

    summary = {
        "source": source,
        "models": results,
    }
    summary_path = out / "benchmark_summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")

    logger.info(f"Benchmark terminé. Résumé: {summary_path}")
    print("\n=== Benchmark Summary ===")
    for r in results:
        print(f"- {r['model']}: {r['duration_seconds']}s | integration={r['integration_score']} | reproducibility={r['reproducibility_score']} | {r['recommendation']}")


if __name__ == "__main__":
    typer.run(main)
