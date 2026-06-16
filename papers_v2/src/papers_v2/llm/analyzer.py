from __future__ import annotations

import json
from typing import Any

from loguru import logger

from papers_v2.analysis.heuristic_mapper import HeuristicArchitecturalMapper
from papers_v2.core.models import (
    AlgorithmAnalysis,
    AnalysisReport,
    ArchitecturalMapping,
    ExperimentPlan,
    ExtractedClaim,
    IntegrationScore,
    MathematicalAnalysis,
    MathematicalObject,
    ReproducibilityScore,
    Risk,
    RiskLevel,
    SystemAnalysis,
)
from papers_v2.llm.client import OllamaClient
from papers_v2.llm.prompts import (
    ARCHITECTURE_PROMPT,
    CONTRIBUTIONS_PROMPT,
    DEEP_ANALYSIS_PROMPT,
    EXECUTIVE_SUMMARY_PROMPT,
    EXPERIMENT_PROMPT,
    GITHUB_ANALYSIS_PROMPT,
    HTML_REPORT_PROMPT,
    MATHEMATICAL_PROMPT,
    PSEUDOCODE_PROMPT,
    RISKS_PROMPT,
    SCORING_PROMPT,
    SYSTEM_ANALYSIS_PROMPT,
)


class LLMAnalyzer:
    def __init__(self, client: OllamaClient | None = None, fallback: bool = True, model: str = "gemma4:e2b") -> None:
        self.client = client or OllamaClient(model=model)
        self.fallback = fallback
        self.heuristic_mapper = HeuristicArchitecturalMapper()

    def _call_llm_json(self, prompt: str, system: str, max_tokens: int = 2048) -> dict[str, Any]:
        if not self.client.is_available():
            logger.warning("Ollama non disponible, fallback heuristique.")
            return {}
        try:
            return self.client.generate_json(prompt, system=system, max_tokens=max_tokens)
        except Exception as e:
            logger.warning(f"Erreur LLM: {e}. Fallback heuristique.")
            return {}

    def analyze_contributions(self, report: AnalysisReport) -> list[str]:
        text = self._prepare_text(report)
        prompt = CONTRIBUTIONS_PROMPT.prompt.format(text=text[:8000])
        result = self._call_llm_json(prompt, CONTRIBUTIONS_PROMPT.system)
        contributions = result.get("contributions", [])
        if not contributions and self.fallback:
            sentences = text.split(".")
            return [s.strip() for s in sentences if any(k in s.lower() for k in ["propose", "introduce", "contribution", "we show"]) and len(s.strip()) > 20][:5]
        return contributions or ["INFORMATION NON DISPONIBLE DANS LE PAPIER"]

    def analyze_executive_summary(self, report: AnalysisReport, contributions: list[str]) -> str:
        prompt = EXECUTIVE_SUMMARY_PROMPT.prompt.format(
            title=report.publication.title,
            abstract=report.publication.abstract or "",
            contributions="\n".join(contributions),
        )
        result = self._call_llm_json(prompt, EXECUTIVE_SUMMARY_PROMPT.system)
        return result.get("executive_summary", "INFORMATION NON DISPONIBLE DANS LE PAPIER")

    def analyze_system(self, report: AnalysisReport) -> SystemAnalysis:
        text = self._prepare_text(report)
        prompt = SYSTEM_ANALYSIS_PROMPT.prompt.format(text=text[:8000])
        result = self._call_llm_json(prompt, SYSTEM_ANALYSIS_PROMPT.system)
        if not result:
            return SystemAnalysis(
                vram_consumption="INFORMATION NON DISPONIBLE DANS LE PAPIER",
                ram_consumption="INFORMATION NON DISPONIBLE DANS LE PAPIER",
                disk_io="INFORMATION NON DISPONIBLE DANS LE PAPIER",
                memory_bandwidth="INFORMATION NON DISPONIBLE DANS LE PAPIER",
                latency="INFORMATION NON DISPONIBLE DANS LE PAPIER",
                throughput="INFORMATION NON DISPONIBLE DANS LE PAPIER",
                scalability="INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )
        return SystemAnalysis(
            vram_consumption=result.get("vram_consumption"),
            ram_consumption=result.get("ram_consumption"),
            disk_io=result.get("disk_io"),
            memory_bandwidth=result.get("memory_bandwidth"),
            latency=result.get("latency"),
            throughput=result.get("throughput"),
            scalability=result.get("scalability"),
            bottlenecks=result.get("bottlenecks", []),
            contention_points=result.get("contention_points", []),
            fragmentation_risks=result.get("fragmentation_risks", []),
        )

    def analyze_architecture(self, report: AnalysisReport) -> ArchitecturalMapping:
        text = self._prepare_text(report)
        prompt = ARCHITECTURE_PROMPT.prompt.format(text=text[:8000])
        result = self._call_llm_json(prompt, ARCHITECTURE_PROMPT.system)
        if not result and self.fallback:
            return self.heuristic_mapper.map(report)
        if not result:
            return ArchitecturalMapping()

        return ArchitecturalMapping(
            perception=result.get("perception", []),
            memory=result.get("memory", []),
            planning=result.get("planning", []),
            decision=result.get("decision", []),
            action=result.get("action", []),
            learning=result.get("learning", []),
            reflection=result.get("reflection", []),
            evaluation=result.get("evaluation", []),
            impacted_modules=result.get("impacted_modules", []),
            required_interfaces=result.get("required_interfaces", []),
            dependencies=result.get("dependencies", []),
        )

    def analyze_risks(self, report: AnalysisReport) -> list[Risk]:
        text = self._prepare_text(report)
        prompt = RISKS_PROMPT.prompt.format(text=text[:8000])
        result = self._call_llm_json(prompt, RISKS_PROMPT.system)
        risks_data = result.get("risks", [])
        risks: list[Risk] = []
        for r in risks_data:
            level_str = r.get("level", "MEDIUM")
            try:
                level = RiskLevel(level_str)
            except ValueError:
                level = RiskLevel.MEDIUM
            risks.append(Risk(
                description=r.get("description", ""),
                level=level,
                mitigation=r.get("mitigation"),
            ))
        return risks

    def analyze_mathematical(self, report: AnalysisReport) -> MathematicalAnalysis:
        text = self._prepare_text(report)
        prompt = MATHEMATICAL_PROMPT.prompt.format(text=text[:8000])
        result = self._call_llm_json(prompt, MATHEMATICAL_PROMPT.system)
        if not result:
            return MathematicalAnalysis()

        variables = [
            MathematicalObject(
                name=v.get("name", ""),
                meaning=v.get("meaning", ""),
                dimensions=v.get("dimensions"),
                domain=v.get("domain"),
            )
            for v in result.get("variables", [])
        ]
        def _normalize_strings(items: list[Any]) -> list[str]:
            normalized: list[str] = []
            for item in items:
                if isinstance(item, str):
                    normalized.append(item)
                elif isinstance(item, dict):
                    normalized.append(" - ".join(str(v) for v in item.values() if v is not None))
                else:
                    normalized.append(str(item))
            return normalized

        return MathematicalAnalysis(
            equations=_normalize_strings(result.get("equations", [])),
            variables=variables,
            loss_functions=_normalize_strings(result.get("loss_functions", [])),
            transformations=_normalize_strings(result.get("transformations", [])),
            uncertainties=_normalize_strings(result.get("uncertainties", [])),
        )

    def analyze_experiment(self, report: AnalysisReport, contributions: list[str]) -> ExperimentPlan:
        prompt = EXPERIMENT_PROMPT.prompt.format(
            title=report.publication.title,
            abstract=report.publication.abstract or "",
            contributions="\n".join(contributions),
        )
        result = self._call_llm_json(prompt, EXPERIMENT_PROMPT.system)
        if not result:
            return ExperimentPlan()
        return ExperimentPlan(
            objective=result.get("objective"),
            hypothesis=result.get("hypothesis"),
            baseline=result.get("baseline"),
            dataset=result.get("dataset"),
            metrics=result.get("metrics", []),
            hardware=result.get("hardware"),
            success_criteria=result.get("success_criteria", []),
            failure_criteria=result.get("failure_criteria", []),
            estimated_duration=result.get("estimated_duration"),
            estimated_cost=result.get("estimated_cost"),
        )

    def analyze_pseudocode(self, report: AnalysisReport, contributions: list[str]) -> AlgorithmAnalysis:
        math = report.mathematical_analysis
        equations = "\n".join(math.equations[:10]) or "INFORMATION NON DISPONIBLE DANS LE PAPIER"
        variables = "\n".join(f"{v.name}: {v.meaning}" for v in math.variables[:10]) or "INFORMATION NON DISPONIBLE DANS LE PAPIER"
        prompt = PSEUDOCODE_PROMPT.prompt.format(
            title=report.publication.title,
            abstract=report.publication.abstract or "",
            contributions="\n".join(contributions),
            equations=equations,
            variables=variables,
        )
        result = self._call_llm_json(prompt, PSEUDOCODE_PROMPT.system)
        if not result:
            return AlgorithmAnalysis()
        from papers_v2.analysis.analyzers import AlgorithmAnalyzer
        base = AlgorithmAnalyzer().analyze(report)
        base.pseudocode = result.get("pseudocode", base.pseudocode)
        base.overall_complexity = result.get("overall_complexity", base.overall_complexity)
        base.memory_cost = result.get("memory_cost", base.memory_cost)
        return base

    def analyze_github(self, github_url: str, repo_info: dict[str, Any]) -> dict[str, Any]:
        prompt = GITHUB_ANALYSIS_PROMPT.prompt.format(
            github_url=github_url,
            repo_info=json.dumps(repo_info, ensure_ascii=False, indent=2),
        )
        result = self._call_llm_json(prompt, GITHUB_ANALYSIS_PROMPT.system)
        return result or {}

    def analyze_scoring(self, report: AnalysisReport) -> tuple[ReproducibilityScore, IntegrationScore] | None:
        text = self._prepare_text(report)
        limitations = "\n".join(report.critical_analysis.get("weaknesses", []))
        prompt = SCORING_PROMPT.prompt.format(
            title=report.publication.title,
            abstract=report.publication.abstract or "",
            contributions="\n".join(report.scientific_contributions),
            limitations=limitations,
            code_available="oui" if report.publication.github_url else "non",
        )
        result = self._call_llm_json(prompt, SCORING_PROMPT.system)
        if not result:
            return None
        try:
            reproducibility = ReproducibilityScore(
                documentation=float(result.get("reproducibility", 0.5)),
                code_available=float(result.get("code_availability", 0.0)),
                data_available=float(result.get("reproducibility", 0.5)),
                results_reproducible=float(result.get("reproducibility", 0.5)),
                acceptable_cost=float(result.get("hardware_cost", 0.5)),
            )
            integration = IntegrationScore(
                reproducibility=reproducibility.score,
                architectural_impact=float(result.get("architectural_impact", 0.5)),
                hardware_cost=float(result.get("hardware_cost", 0.5)),
                code_availability=float(result.get("code_availability", 0.0)),
                scientific_maturity=float(result.get("scientific_maturity", 0.5)),
            )
            return reproducibility, integration
        except (ValueError, TypeError) as e:
            logger.warning(f"Scores LLM invalides: {e}")
            return None

    def analyze_deep(self, report: AnalysisReport) -> dict[str, Any]:
        text = self._prepare_text(report)
        prompt = DEEP_ANALYSIS_PROMPT.prompt.format(
            title=report.publication.title,
            abstract=report.publication.abstract or "",
            text=text[:12000],
        )
        return self._call_llm_json(prompt, DEEP_ANALYSIS_PROMPT.system) or {}

    def generate_html_summary(self, report: AnalysisReport) -> str:
        # Build a compact structured summary instead of full JSON dump
        summary = {
            "id": report.publication.id,
            "title": report.publication.title,
            "source": report.publication.source,
            "integration_score": report.integration.score,
            "reproducibility_score": report.reproducibility.score,
            "recommendation": report.recommendation.value,
            "executive_summary": report.executive_summary,
            "contributions": report.scientific_contributions,
            "architecture": report.architectural_mapping.model_dump(),
            "experiment": report.experiment_plan.model_dump(),
            "risks": [r.model_dump() for r in report.risks],
        }
        report_json = json.dumps(summary, ensure_ascii=False, indent=2)
        report_json = report_json[:12000]
        prompt = HTML_REPORT_PROMPT.prompt.format(report_json=report_json)
        # HTML generation: do not use json_mode to avoid HTML-in-JSON issues
        if not self.client.is_available():
            return "<html><body>HTML non généré (Ollama indisponible)</body></html>"
        try:
            raw = self.client.generate(prompt, system=HTML_REPORT_PROMPT.system, max_tokens=4096)
            return self._extract_html(raw)
        except Exception as e:
            logger.warning(f"Erreur génération HTML: {e}")
            return "<html><body>HTML non généré</body></html>"

    @staticmethod
    def _extract_html(text: str) -> str:
        text = text.strip()
        # Strip markdown code fence if present
        if text.startswith("```"):
            lines = text.splitlines()
            if lines[0].startswith("```"):
                lines = lines[1:]
            if lines and lines[-1].startswith("```"):
                lines = lines[:-1]
            text = "\n".join(lines).strip()
        if "<html" in text.lower():
            start = text.lower().find("<html")
            return text[start:]
        return f"<html><body>{text}</body></html>"

    def _prepare_text(self, report: AnalysisReport) -> str:
        parts = [
            report.publication.title,
            report.publication.abstract or "",
            report.publication.full_text or "",
        ]
        return "\n\n".join(p for p in parts if p)
