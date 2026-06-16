from __future__ import annotations

from typing import Any

from loguru import logger

from papers_v2.llm.client import OllamaClient

ANALYZER_SYSTEM = (
    "You are an expert scientific analyst. Your role is to distill complex experimental "
    "outcomes into structured, actionable insights that will guide future iterations of an "
    "evolutionary research system. Be precise, critical, and constructive."
)

ANALYZER_PROMPT = (
    "## Candidate Program\n"
    "Motivation: {motivation}\n"
    "```\n{program_summary}\n```\n\n"
    "## Experimental Results\n"
    "Score: {score}\n"
    "Success: {success}\n"
    "Metrics: {metrics}\n"
    "Runtime: {runtime_seconds}s\n"
    "Stdout (excerpt): {stdout}\n"
    "Stderr (excerpt): {stderr}\n\n"
    "## Instructions\n"
    "Analyze the experimental results above and produce a JSON object with:\n"
    "- \"summary\": A concise 2-3 sentence summary of what happened.\n"
    "- \"strengths\": List of what worked well in this candidate.\n"
    "- \"weaknesses\": List of what failed or underperformed.\n"
    "- \"root_cause\": Your best hypothesis for WHY the result turned out this way.\n"
    "- \"actionable_insights\": Specific, concrete suggestions for the next iteration "
    "(what to try, what to avoid, what to preserve).\n"
    "- \"coverage_analysis\": Which aspects of the task were well-addressed vs. neglected.\n"
    "- \"novelty_assessment\": How novel is this approach compared to typical solutions? "
    "(score 0-10 and brief justification).\n"
    "- \"cognition_update\": A knowledge item to add to the cognition base "
    "(a lesson learned, a heuristic, or a design principle). Empty string if nothing new.\n"
)


class EvolutionAnalyzer:
    def __init__(self, model: str = "gemma4:e2b", use_llm: bool = True) -> None:
        self.model = model
        self.use_llm = use_llm
        self.client = OllamaClient(model=model) if use_llm else None

    def analyze(
        self,
        motivation: str,
        program: str,
        results: dict[str, Any],
        max_tokens: int = 2048,
    ) -> dict[str, Any]:
        metrics_str = str(results.get("metrics", {}))
        stdout = str(results.get("stdout", ""))[:2000]
        stderr = str(results.get("stderr", ""))[:2000]

        prompt = ANALYZER_PROMPT.format(
            motivation=str(motivation)[:1500],
            program_summary=str(program)[:2000],
            score=results.get("score", "N/A"),
            success=results.get("success", False),
            metrics=metrics_str[:1500],
            runtime_seconds=results.get("runtime_seconds", 0),
            stdout=stdout,
            stderr=stderr,
        )

        if not self.use_llm or not self.client or not self.client.is_available():
            return self._heuristic_analysis(results)

        try:
            analysis = self.client.generate_json(
                prompt,
                system=ANALYZER_SYSTEM,
                max_tokens=max_tokens,
            )
            return {
                "summary": analysis.get("summary", ""),
                "strengths": analysis.get("strengths", []),
                "weaknesses": analysis.get("weaknesses", []),
                "root_cause": analysis.get("root_cause", ""),
                "actionable_insights": analysis.get("actionable_insights", []),
                "coverage_analysis": analysis.get("coverage_analysis", ""),
                "novelty_assessment": analysis.get("novelty_assessment", {}),
                "cognition_update": analysis.get("cognition_update", ""),
            }
        except Exception as e:
            logger.warning(f"Erreur LLM dans Analyzer: {e}")
            return self._heuristic_analysis(results)

    def _heuristic_analysis(self, results: dict[str, Any]) -> dict[str, Any]:
        success = results.get("success", False)
        score = results.get("score", 0.0)
        error = results.get("error", "")

        return {
            "summary": (
                f"Experiment {'succeeded' if success else 'failed'} with score {score}. "
                + (f"Error: {error[:200]}" if error else "No errors reported.")
            ),
            "strengths": ["Execution completed"] if success else [],
            "weaknesses": [f"Error: {error[:200]}"] if error else ["Score below threshold"],
            "root_cause": error if error else "Insufficient data for root cause analysis",
            "actionable_insights": [
                "Review error traces and fix implementation issues." if error
                else "Consider architectural improvements for higher score."
            ],
            "coverage_analysis": "Heuristic analysis: limited coverage assessment.",
            "novelty_assessment": {"score": 5, "justification": "Unable to assess without LLM."},
            "cognition_update": "",
        }

    def analyze_batch(
        self,
        candidates: list[dict[str, Any]],
        results_list: list[dict[str, Any]],
    ) -> list[dict[str, Any]]:
        analyses = []
        for i, (candidate, results) in enumerate(zip(candidates, results_list)):
            analysis = self.analyze(
                motivation=candidate.get("motivation", ""),
                program=candidate.get("program", ""),
                results=results,
            )
            analyses.append(analysis)
        return analyses
