# ⚠️  Migrated to Rust (papers_core/src/engine.rs) — stub only
from __future__ import annotations

import re
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
from papers_v2.llm.analyzer import LLMAnalyzer


# Modules deleted (ported to Rust) — stubs inline
class Transpiler:
    def transpile(self, report): return type("obj", (), {"structs": [], "interfaces": [], "apis": [], "algorithms": []})()

class ExperimentPlanner:
    def plan(self, report): return type("obj", (), {"objective": None})()

class TestPlanner:
    def plan(self, report): return type("obj", (), {"unit_tests": [], "integration_tests": [], "regression_tests": []})()

class GitHubExtractor:
    def extract_repo_info(self, url): return {}


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
        report = AnalysisReport(publication=publication)
        report.scientific_contributions = self._extract_contributions(report)
        report.executive_summary = self._generate_executive_summary(report, report.scientific_contributions)
        report.mathematical_analysis = self._analyze_mathematical(report)
        report.algorithm_analysis = self._analyze_pseudocode(report, report.scientific_contributions)
        report.system_analysis = self._analyze_system(report)
        report.architectural_mapping = self._analyze_architecture(report)
        report.software_specification = self.transpiler.transpile(report)
        report.experiment_plan = self._analyze_experiment(report, report.scientific_contributions)
        report.test_plan = self.test_planner.plan(report)
        repro, integration = self._analyze_scoring(report)
        report.reproducibility = repro
        report.integration = integration
        report.risks = self._analyze_risks(report)
        report.critical_analysis = self._critical_analysis(report)
        report.recommendation = self._determine_recommendation(report)
        report.recommendation_justification = self._recommendation_justification(report)
        report.claims = self._extract_claims(report)
        if deep and self.llm_analyzer and self.use_llm:
            self.deep_analysis = self.llm_analyzer.analyze_deep(report)
        return report

    def _extract_contributions(self, report):
        if self.llm_analyzer and self.use_llm:
            c = self.llm_analyzer.analyze_contributions(report)
            if c and c != ["INFORMATION NON DISPONIBLE DANS LE PAPIER"]:
                return c
        return ["Contribution à extraire manuellement."]

    def _generate_executive_summary(self, report, contributions):
        if self.llm_analyzer and self.use_llm:
            s = self.llm_analyzer.analyze_executive_summary(report, contributions)
            if s and s != "INFORMATION NON DISPONIBLE DANS LE PAPIER":
                return s
        return f"Papier '{report.publication.title}' — analyse préliminaire."

    def _analyze_system(self, report):
        return self.llm_analyzer.analyze_system(report) if (self.llm_analyzer and self.use_llm) else self.system_analyzer.analyze(report)

    def _analyze_architecture(self, report):
        if self.llm_analyzer and self.use_llm:
            m = self.llm_analyzer.analyze_architecture(report)
            if m.impacted_modules: return m
        return self.arch_mapper.map(report)

    def _analyze_risks(self, report):
        return self.llm_analyzer.analyze_risks(report) if (self.llm_analyzer and self.use_llm) else self.risk_analyzer.analyze(report)

    def _analyze_mathematical(self, report):
        return self.llm_analyzer.analyze_mathematical(report) if (self.llm_analyzer and self.use_llm) else self.math_analyzer.analyze(report)

    def _analyze_pseudocode(self, report, contributions):
        return self.llm_analyzer.analyze_pseudocode(report, contributions) if (self.llm_analyzer and self.use_llm) else self.algo_analyzer.analyze(report)

    def _analyze_experiment(self, report, contributions):
        if self.llm_analyzer and self.use_llm:
            p = self.llm_analyzer.analyze_experiment(report, contributions)
            if p.objective: return p
        return self.exp_planner.plan(report)

    def _analyze_scoring(self, report):
        if self.llm_analyzer and self.use_llm:
            r = self.llm_analyzer.analyze_scoring(report)
            if r: return r
        return self.scoring.compute_reproducibility(report), self.scoring.compute_integration(report)

    def _critical_analysis(self, report):
        return {"strengths": ["Potentiellement reproductible."], "weaknesses": ["Analyse textuelle partielle."]}

    def _determine_recommendation(self, report):
        s = report.integration.score
        if s <= 0.30: return Recommendation.REJECT
        elif s <= 0.50: return Recommendation.ARCHIVE
        elif s <= 0.70: return Recommendation.PROTOTYPE
        return Recommendation.INTEGRATION

    def _recommendation_justification(self, report):
        return f"Score d'intégration: {report.integration.score:.2f}."

    def _extract_claims(self, report):
        return [ExtractedClaim(text="Analyse préliminaire.", claim_type="[UNCERTAINTY]")]
