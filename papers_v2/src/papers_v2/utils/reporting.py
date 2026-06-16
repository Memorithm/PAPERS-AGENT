from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any

from jinja2 import Template
from loguru import logger

from papers_v2.core.models import AnalysisReport, Recommendation


REPORT_TEMPLATE = """---
id: {{ report.publication.id }}
title: {{ report.publication.title }}
authors: {{ report.publication.authors | map(attribute='name') | join(', ') }}
publication_date: {{ report.publication.publication_date or 'Non disponible' }}
source: {{ report.publication.source or 'Non disponible' }}
domains: {{ report.publication.domains | join(', ') }}
integration_score: {{ '%.2f' | format(report.integration.score) }}
reproducibility_score: {{ '%.2f' | format(report.reproducibility.score) }}
code_available: {{ 'Oui' if report.publication.github_url else 'Non' }}
github: {{ report.publication.github_url or 'Non disponible' }}
paper_url: {{ report.publication.paper_url or 'Non disponible' }}
---

# Résumé Exécutif

{{ report.executive_summary or 'Non disponible' }}

# Contributions Scientifiques

{% for contrib in report.scientific_contributions %}
- {{ contrib }}
{% else %}
Aucune contribution extraite.
{% endfor %}

# Analyse Mathématique

## Équations

{% for eq in report.mathematical_analysis.equations %}
- {{ eq }}
{% else %}
Aucune équation extraite.
{% endfor %}

## Variables

{% for var in report.mathematical_analysis.variables %}
- **{{ var.name }}** : {{ var.meaning }}
{% else %}
Aucune variable extraite.
{% endfor %}

## Incertitudes

{% for u in report.mathematical_analysis.uncertainties %}
- {{ u }}
{% else %}
Aucune.
{% endfor %}

# Analyse Algorithmique

## Pseudo-code

```text
{{ report.algorithm_analysis.pseudocode or 'Non disponible' }}
```

## Complexité

- **Complexité globale** : {{ report.algorithm_analysis.overall_complexity or 'Non disponible' }}
- **Coût mémoire** : {{ report.algorithm_analysis.memory_cost or 'Non disponible' }}

# Analyse Système

| Ressource | Valeur |
|-----------|--------|
| VRAM | {{ report.system_analysis.vram_consumption or 'Non disponible' }} |
| RAM | {{ report.system_analysis.ram_consumption or 'Non disponible' }} |
| I/O disque | {{ report.system_analysis.disk_io or 'Non disponible' }} |
| Bande passante mémoire | {{ report.system_analysis.memory_bandwidth or 'Non disponible' }} |
| Latence | {{ report.system_analysis.latency or 'Non disponible' }} |
| Débit | {{ report.system_analysis.throughput or 'Non disponible' }} |
| Scalabilité | {{ report.system_analysis.scalability or 'Non disponible' }} |

# Architecture du Flux

```mermaid
flowchart TD
    INPUT[Entrée] --> PRE[Prétraitement]
    PRE --> PROC[Calcul principal]
    PROC --> MEM[Mémoire]
    MEM --> OUT[Sortie]
```

# Traduction Logicielle

## Structures

{% for s in report.software_specification.structs %}
### {{ s.language }}
{{ s.code }}
{% endfor %}

## Interfaces

{% for i in report.software_specification.interfaces %}
### {{ i.language }}
{{ i.code }}
{% endfor %}

## API

{% for a in report.software_specification.apis %}
### {{ a.language }}
{{ a.code }}
{% endfor %}

## Algorithmes

{% for a in report.software_specification.algorithms %}
### {{ a.language }}
{{ a.code }}
{% endfor %}

# Plan d'Expérimentation

- **Objectif** : {{ report.experiment_plan.objective or 'Non disponible' }}
- **Hypothèse** : {{ report.experiment_plan.hypothesis or 'Non disponible' }}
- **Baseline** : {{ report.experiment_plan.baseline or 'Non disponible' }}
- **Dataset** : {{ report.experiment_plan.dataset or 'Non disponible' }}
- **Métriques** : {{ report.experiment_plan.metrics | join(', ') or 'Non disponibles' }}
- **Matériel** : {{ report.experiment_plan.hardware or 'Non disponible' }}
- **Critères de succès** : {{ report.experiment_plan.success_criteria | join('; ') or 'Non disponis' }}
- **Critères d'échec** : {{ report.experiment_plan.failure_criteria | join('; ') or 'Non disponibles' }}
- **Durée estimée** : {{ report.experiment_plan.estimated_duration or 'Non disponible' }}
- **Coût estimé** : {{ report.experiment_plan.estimated_cost or 'Non disponible' }}

# Plan de Tests

## Unitaires

{% for t in report.test_plan.unit_tests %}
- {{ t }}
{% endfor %}

## Intégration

{% for t in report.test_plan.integration_tests %}
- {{ t }}
{% endfor %}

## Régression

{% for t in report.test_plan.regression_tests %}
- {{ t }}
{% endfor %}

# Analyse Critique

## Forces

{% for s in report.critical_analysis.get('strengths', []) %}
- {{ s }}
{% else %}
Aucune force identifiée.
{% endfor %}

## Faiblesses

{% for w in report.critical_analysis.get('weaknesses', []) %}
- {{ w }}
{% else %}
Aucune faiblesse identifiée.
{% endfor %}

## Risques

{% for r in report.risks %}
- **{{ r.level }}** : {{ r.description }} (Atténuation : {{ r.mitigation or 'Non définie' }})
{% else %}
Aucun risque identifié.
{% endfor %}

## Zones d'incertitude

{% for c in report.claims if c.claim_type == '[UNCERTAINTY]' %}
- {{ c.text }}
{% else %}
Aucune.
{% endfor %}

# Compatibilité Architecture Agentique

- **Mémoire** : {{ report.architectural_mapping.memory | join(', ') or 'Non impacté' }}
- **Raisonnement** : {{ report.architectural_mapping.decision | join(', ') or 'Non impacté' }}
- **Planification** : {{ report.architectural_mapping.planning | join(', ') or 'Non impacté' }}
- **Action** : {{ report.architectural_mapping.action | join(', ') or 'Non impacté' }}
- **Réflexion** : {{ report.architectural_mapping.reflection | join(', ') or 'Non impacté' }}
- **Auto-amélioration** : {{ report.architectural_mapping.learning | join(', ') or 'Non impacté' }}

# Recommandation Finale

**{{ report.recommendation.value }}**

{{ report.recommendation_justification or 'Aucune justification fournie.' }}

Interprétation du score d'intégration : **{{ report.integration.interpretation }}**
"""


class ReportRenderer:
    def render(self, report: AnalysisReport) -> str:
        template = Template(REPORT_TEMPLATE)
        return template.render(report=report)

    def save(self, report: AnalysisReport, path: str | Path) -> Path:
        output = Path(path)
        output.write_text(self.render(report), encoding="utf-8")
        logger.info(f"Rapport sauvegardé: {output}")
        return output
