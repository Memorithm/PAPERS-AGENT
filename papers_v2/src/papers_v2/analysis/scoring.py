from __future__ import annotations

from loguru import logger

from papers_v2.core.models import (
    AnalysisReport,
    IntegrationScore,
    ReproducibilityScore,
    Risk,
    RiskLevel,
)


class ScoringEngine:
    def compute_reproducibility(self, report: AnalysisReport) -> ReproducibilityScore:
        logger.info("Calcul du score de reproductibilité...")
        publication = report.publication

        documentation = 0.5
        code_available = 1.0 if publication.github_url else 0.0
        data_available = 0.5
        results_reproducible = 0.5
        acceptable_cost = 0.5

        if publication.full_text and len(publication.full_text) > 10000:
            documentation = 0.8

        return ReproducibilityScore(
            documentation=documentation,
            code_available=code_available,
            data_available=data_available,
            results_reproducible=results_reproducible,
            acceptable_cost=acceptable_cost,
        )

    def compute_integration(self, report: AnalysisReport) -> IntegrationScore:
        logger.info("Calcul du score d'intégration...")
        repro = report.reproducibility.score
        return IntegrationScore(
            reproducibility=repro,
            architectural_impact=0.5,
            hardware_cost=0.5,
            code_availability=report.publication.github_url is not None and 1.0 or 0.0,
            scientific_maturity=0.5,
        )


class RiskAnalyzer:
    def analyze(self, report: AnalysisReport) -> list[Risk]:
        logger.info("Analyse des risques...")
        risks: list[Risk] = []
        if not report.publication.github_url:
            risks.append(Risk(
                description="Aucun code source n'est associé à la publication.",
                level=RiskLevel.MEDIUM,
                mitigation="Contacter les auteurs ou tenter une reproduction indépendante.",
            ))
        if not report.publication.full_text:
            risks.append(Risk(
                description="Texte complet non disponible, l'analyse repose sur l'abstract.",
                level=RiskLevel.HIGH,
                mitigation="Récupérer le PDF complet pour une analyse approfondie.",
            ))
        return risks
