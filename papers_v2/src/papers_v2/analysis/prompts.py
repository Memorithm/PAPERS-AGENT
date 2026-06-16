from __future__ import annotations

from dataclasses import dataclass


@dataclass
class PromptTemplate:
    name: str
    system: str
    user: str


CONTRIBUTIONS_PROMPT = PromptTemplate(
    name="contributions",
    system="Tu es un ingénieur de recherche. Extrais les contributions scientifiques principales d'un papier.",
    user="""Texte du papier (extrait):
{text}

---
Liste les contributions scientifiques principales sous forme de bullet points concis. N'invente rien.""",
)

EXECUTIVE_SUMMARY_PROMPT = PromptTemplate(
    name="executive_summary",
    system="Tu es un architecte IA. Résume l'opportunité d'intégration d'une technique dans un système IA autonome local/serveur.",
    user="""Titre: {title}
Abstract: {abstract}
Contributions: {contributions}

---
Rédige un résumé exécutif de 3 paragraphes maximum:
1. Quel est l'objectif et l'hypothèse du papier ?
2. Quelle est la méthode clé et son impact architecture agentique ?
3. Quelles sont les limites et le niveau de confiance pour une intégration ?""",
)

SYSTEM_PROMPT = PromptTemplate(
    name="system_analysis",
    system="Tu estimes les coûts système d'une méthode IA sur une configuration cible: GPU RTX 4090/5090 24 Go, 64-256 Go RAM, NVMe, Linux.",
    user="""Texte du papier:
{text}

---
Estime sous forme de bullet points:
- Consommation VRAM (Go)
- Consommation RAM (Go)
- I/O disque
- Bande passante mémoire
- Latence
- Débit
- Scalabilité
- Bottlenecks
- Points de contention
- Risques de fragmentation mémoire

Si une information est absente, écris: INFORMATION NON DISPONIBLE DANS LE PAPIER.""",
)

ARCHITECTURE_PROMPT = PromptTemplate(
    name="architecture_mapping",
    system="Tu cartographies une technique de recherche sur l'architecture d'un agent IA autonome.",
    user="""Texte:
{text}

---
Pour chaque pilier d'agent IA, indique si et comment le papier l'impacte. Utilise des bullet points vides si non applicable:
- Perception
- Mémoire
- Planification
- Décision
- Action
- Apprentissage
- Réflexion
- Évaluation

Puis liste:
- Modules impactés
- Interfaces nécessaires
- Dépendances""",
)

RISKS_PROMPT = PromptTemplate(
    name="risks",
    system="Tu détectes les risques d'une technique de ML pour un système IA autonome en production locale.",
    user="""Texte:
{text}

---
Identifie les risques possibles (overfitting, dépendances cachées, fuite de données, coûts sous-estimés, problèmes d'échelle, hypothèses irréalistes). Attribue un niveau LOW/MEDIUM/HIGH/CRITICAL et une mitigation.""",
)
