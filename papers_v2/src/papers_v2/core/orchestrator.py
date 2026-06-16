from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from loguru import logger

from papers_v2.analysis.analyzers import (
    AlgorithmAnalyzer,
    MathematicalAnalyzer,
    SystemAnalyzer,
)
from papers_v2.analysis.heuristic_mapper import HeuristicArchitecturalMapper
from papers_v2.analysis.scoring import RiskAnalyzer, ScoringEngine
from papers_v2.core.models import (
    AnalysisReport,
    ExtractedClaim,
    Publication,
    Recommendation,
)
from papers_v2.extraction.code_extractors import GitHubExtractor
from papers_v2.experimentation.planner import ExperimentPlanner, TestPlanner
from papers_v2.llm.analyzer import LLMAnalyzer
from papers_v2.transpilation.transpiler import Transpiler


class PapersEngine:
    def __init__(self, use_llm: bool = True, llm_model: str = "gemma4:e2b") -> None:
        self.use_llm = use_llm
        self.llm_analyzer = LLMAnalyzer(model=llm_model) if use_llm else None
        self.github_extractor = GitHubExtractor()
        self.math_analyzer = MathematicalAnalyzer()
        self.algo_analyzer = AlgorithmAnalyzer()
        self.system_analyzer = SystemAnalyzer()
        self.arch_mapper = HeuristicArchitecturalMapper()
        self.scoring = ScoringEngine()
        self.risk_analyzer = RiskAnalyzer()
        self.transpiler = Transpiler()
        self.exp_planner = ExperimentPlanner()
        self.test_planner = TestPlanner()
        self.deep_analysis: dict[str, Any] = {}

    def analyze(self, publication: Publication, deep: bool = False) -> AnalysisReport:
        logger.info(f"Analyse de: {publication.title}")

        report = AnalysisReport(publication=publication)

        # Phase 1: enrichissement avec GitHub si disponible
        if publication.github_url:
            try:
                repo_info = self.github_extractor.extract_repo_info(publication.github_url)
                publication.github_url = publication.github_url
                # Stocker les métadonnées GitHub dans le rapport via metadata custom
                report.critical_analysis.setdefault("github_metadata", repo_info)
            except Exception as e:
                logger.warning(f"Impossible d'analyser GitHub: {e}")

        report.scientific_contributions = self._extract_contributions(report)
        report.executive_summary = self._generate_executive_summary(report, report.scientific_contributions)
        report.mathematical_analysis = self._analyze_mathematical(report)
        report.algorithm_analysis = self._analyze_pseudocode(report, report.scientific_contributions)
        report.system_analysis = self._analyze_system(report)
        report.architectural_mapping = self._analyze_architecture(report)
        report.software_specification = self.transpiler.transpile(report)
        report.experiment_plan = self._analyze_experiment(report, report.scientific_contributions)
        report.test_plan = self.test_planner.plan(report)

        # Phase 2: scoring (heuristique ou LLM)
        repro, integration = self._analyze_scoring(report)
        report.reproducibility = repro
        report.integration = integration

        report.risks = self._analyze_risks(report)
        report.critical_analysis = self._critical_analysis(report)
        report.recommendation = self._determine_recommendation(report)
        report.recommendation_justification = self._recommendation_justification(report)
        report.claims = self._extract_claims(report)

        # Phase 3: deep analysis optionnelle
        if deep and self.llm_analyzer and self.use_llm:
            self.deep_analysis = self.llm_analyzer.analyze_deep(report)

        return report

    def _extract_contributions(self, report: AnalysisReport) -> list[str]:
        if self.llm_analyzer and self.use_llm:
            contributions = self.llm_analyzer.analyze_contributions(report)
            if contributions and contributions != ["INFORMATION NON DISPONIBLE DANS LE PAPIER"]:
                return contributions
        text = report.publication.full_text or report.publication.abstract or ""
        sentences = re.split(r"(?<=[.!?])\s+", text)
        candidates = [
            s.strip() for s in sentences
            if any(kw in s.lower() for kw in ["propose", "introduce", "present", "contribution", "we show", "we demonstrate"])
            and len(s.strip()) > 30
        ]
        if candidates:
            return candidates[:5]
        return ["Contribution principale à extraire manuellement à partir du texte complet."]

    def _generate_executive_summary(self, report: AnalysisReport, contributions: list[str]) -> str:
        if self.llm_analyzer and self.use_llm:
            summary = self.llm_analyzer.analyze_executive_summary(report, contributions)
            if summary and summary != "INFORMATION NON DISPONIBLE DANS LE PAPIER":
                return summary
        return (
            f"Le papier '{report.publication.title}' est analysé dans le cadre de l'amélioration "
            f"d'une architecture IA autonome locale/serveur. L'évaluation est préliminaire et "
            f"requiert un examen humain complémentaire."
        )

    def _analyze_system(self, report: AnalysisReport):
        if self.llm_analyzer and self.use_llm:
            return self.llm_analyzer.analyze_system(report)
        return self.system_analyzer.analyze(report)

    def _analyze_architecture(self, report: AnalysisReport):
        if self.llm_analyzer and self.use_llm:
            mapping = self.llm_analyzer.analyze_architecture(report)
            if mapping.impacted_modules:
                return mapping
        return self.arch_mapper.map(report)

    def _analyze_risks(self, report: AnalysisReport):
        if self.llm_analyzer and self.use_llm:
            risks = self.llm_analyzer.analyze_risks(report)
            if risks:
                return risks
        return self.risk_analyzer.analyze(report)

    def _analyze_mathematical(self, report: AnalysisReport):
        if self.llm_analyzer and self.use_llm:
            return self.llm_analyzer.analyze_mathematical(report)
        return self.math_analyzer.analyze(report)

    def _analyze_pseudocode(self, report: AnalysisReport, contributions: list[str]):
        if self.llm_analyzer and self.use_llm:
            return self.llm_analyzer.analyze_pseudocode(report, contributions)
        return self.algo_analyzer.analyze(report)

    def _analyze_experiment(self, report: AnalysisReport, contributions: list[str]):
        if self.llm_analyzer and self.use_llm:
            plan = self.llm_analyzer.analyze_experiment(report, contributions)
            if plan.objective:
                return plan
        return self.exp_planner.plan(report)

    def _analyze_scoring(self, report: AnalysisReport):
        if self.llm_analyzer and self.use_llm:
            result = self.llm_analyzer.analyze_scoring(report)
            if result:
                return result
        return self.scoring.compute_reproducibility(report), self.scoring.compute_integration(report)

    def _critical_analysis(self, report: AnalysisReport) -> dict[str, list[str] | Any]:
        strengths = ["Travail potentiellement reproductible si le code est disponible."]
        weaknesses = ["Analyse basée sur un texte partiel."]
        github_metadata = report.critical_analysis.get("github_metadata", {})
        if github_metadata:
            if github_metadata.get("has_readme"):
                strengths.append("README présent dans le dépôt.")
            if github_metadata.get("has_tests"):
                strengths.append("Tests présents dans le dépôt.")
            if not github_metadata.get("has_readme"):
                weaknesses.append("Absence de README détectée.")
        return {"strengths": strengths, "weaknesses": weaknesses}

    def _determine_recommendation(self, report: AnalysisReport) -> Recommendation:
        score = report.integration.score
        if score <= 0.30:
            return Recommendation.REJECT
        elif score <= 0.50:
            return Recommendation.ARCHIVE
        elif score <= 0.70:
            return Recommendation.PROTOTYPE
        else:
            return Recommendation.INTEGRATION

    def _recommendation_justification(self, report: AnalysisReport) -> str:
        return (
            f"Score d'intégration de {report.integration.score:.2f} "
            f"({report.integration.interpretation}). Décision à affiner après lecture complète."
        )

    def _extract_claims(self, report: AnalysisReport) -> list[ExtractedClaim]:
        return [
            ExtractedClaim(
                text="L'analyse ci-dessus est préliminaire.",
                claim_type="[UNCERTAINTY]",
            )
        ]
