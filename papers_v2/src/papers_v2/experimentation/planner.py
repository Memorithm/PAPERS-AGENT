from __future__ import annotations

from loguru import logger

from papers_v2.core.models import AnalysisReport, ExperimentPlan, TestPlan


class ExperimentPlanner:
    def plan(self, report: AnalysisReport) -> ExperimentPlan:
        logger.info("Création du plan d'expérimentation...")
        return ExperimentPlan(
            objective=f"Valider l'intégration de la technique décrite dans {report.publication.title}",
            hypothesis="La technique améliore significativement les performances de l'architecture cible.",
            baseline="Architecture de référence sans la technique proposée.",
            dataset="Dataset public pertinent (à définir après lecture complète)",
            metrics=["latence", "débit", "utilisation VRAM", "précision"],
            hardware="GPU ≥ 24 Go VRAM, 64-256 Go RAM, NVMe",
            success_criteria=["Amélioration ≥ 5% sur la métrique principale"],
            failure_criteria=["Régression > 2%", "Coût mémoire doublé"],
            estimated_duration="1-2 semaines",
            estimated_cost="Coût électricité GPU",
        )


class TestPlanner:
    def plan(self, report: AnalysisReport) -> TestPlan:
        logger.info("Création du plan de tests...")
        return TestPlan(
            unit_tests=[
                "Vérifier les dimensions des tenseurs intermédiaires",
                "Tester la stabilité numérique",
                "Tester les cas limites (entrées vides, zéros, NaN)",
            ],
            integration_tests=[
                "Mesurer le débit sous charge",
                "Mesurer la latence de bout en bout",
                "Vérifier la persistance de l'état",
            ],
            regression_tests=[
                "Comparer version baseline vs version modifiée",
                "S'assurer de l'absence de régression mémoire",
            ],
        )
