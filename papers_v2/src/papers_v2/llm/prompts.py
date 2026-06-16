"""Prompts structurés pour l'analyse de papier par LLM local.

Chaque prompt demande une sortie JSON exploitable par PAPERS V2.
"""

from __future__ import annotations

from dataclasses import dataclass


SYSTEM_PAPERS_ANALYST = """Tu es PAPERS, un agent de recherche appliquée spécialisé dans l'analyse de publications scientifiques pour architectures IA autonomes.

RÈGLES ABSOLUES:
- Ne jamais inventer une information.
- Si une information est absente du texte, réponds "INFORMATION NON DISPONIBLE DANS LE PAPIER".
- Toutes les affirmations doivent être fondées sur le texte fourni.
- Réponds UNIQUEMENT en JSON valide, sans markdown, sans explications hors JSON.
"""


@dataclass
class LLMPrompt:
    system: str
    prompt: str


CONTRIBUTIONS_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Analyse le texte suivant d'une publication scientifique.

EXTRAIS les contributions scientifiques principales sous forme de liste.

Texte:
{text}

Réponds en JSON strict:
{{
  "contributions": ["contribution 1", "contribution 2", ...]
}}

Si aucune contribution claire n'est identifiable, returns {{"contributions": ["INFORMATION NON DISPONIBLE DANS LE PAPIER"]}}.""",
)


EXECUTIVE_SUMMARY_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Rédige un résumé exécutif de 3 paragraphes maximum pour la publication suivante.

Titre: {title}
Abstract: {abstract}
Contributions: {contributions}

Réponds en JSON strict:
{{
  "executive_summary": "texte du résumé"
}}

Le résumé doit couvrir:
1. Objectif et hypothèse
2. Méthode clé et impact architecture agentique
3. Limites et niveau de confiance pour intégration""",
)


SYSTEM_ANALYSIS_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Analyse le texte d'un papier et estime les coûts système sur une cible: GPU RTX 4090/5090 (24 Go), 64-256 Go RAM, NVMe, Linux.

Texte:
{text}

Réponds en JSON strict:
{{
  "vram_consumption": "estimation ou INFORMATION NON DISPONIBLE DANS LE PAPIER",
  "ram_consumption": "...",
  "disk_io": "...",
  "memory_bandwidth": "...",
  "latency": "...",
  "throughput": "...",
  "scalability": "...",
  "bottlenecks": ["..."],
  "contention_points": ["..."],
  "fragmentation_risks": ["..."]
}}""",
)


ARCHITECTURE_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Cartographie la technique du papier sur l'architecture d'un agent IA autonome.

Texte:
{text}

Réponds en JSON strict:
{{
  "perception": ["mot-clés ou phrases"],
  "memory": ["..."],
  "planning": ["..."],
  "decision": ["..."],
  "action": ["..."],
  "learning": ["..."],
  "reflection": ["..."],
  "evaluation": ["..."],
  "impacted_modules": ["memory_module", "reasoning_module", ...],
  "required_interfaces": ["..."],
  "dependencies": ["PyTorch", "CUDA", ...]
}}

Utilise [] si un pilier n'est pas impacté.""",
)


RISKS_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Identifie les risques de la technique décrite dans le texte pour un système IA autonome en production locale.

Texte:
{text}

Réponds en JSON strict:
{{
  "risks": [
    {{
      "description": "description du risque",
      "level": "LOW|MEDIUM|HIGH|CRITICAL",
      "mitigation": "action d'atténuation"
    }}
  ]
}}

Si aucun risque identifiable, returns {{"risks": []}}.""",
)


MATHEMATICAL_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Extrais les éléments mathématiques du texte.

Texte:
{text}

Réponds en JSON strict:
{{
  "equations": ["$E=mc^2$", ...],
  "variables": [
    {{"name": "M_t", "meaning": "état mémoire au temps t", "dimensions": "d_model x 1", "domain": "R"}}
  ],
  "loss_functions": ["..."],
  "transformations": ["..."],
  "uncertainties": ["..."]
}}

N'invente pas d'équations. Utilise "INFORMATION NON DISPONIBLE DANS LE PAPIER" quand nécessaire.""",
)


EXPERIMENT_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Propose un plan d'expérimentation pour valider l'intégration de la technique dans une architecture IA autonome.

Titre: {title}
Abstract: {abstract}
Contributions: {contributions}

Réponds en JSON strict:
{{
  "objective": "...",
  "hypothesis": "...",
  "baseline": "...",
  "dataset": "...",
  "metrics": ["latence", "..."],
  "hardware": "GPU ≥ 24 Go VRAM, 64-256 Go RAM, NVMe",
  "success_criteria": ["..."],
  "failure_criteria": ["..."],
  "estimated_duration": "...",
  "estimated_cost": "..."
}}""",
)


PSEUDOCODE_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Génère un pseudo-code détaillé pour implémenter la technique décrite.

Titre: {title}
Abstract: {abstract}
Contributions: {contributions}
Équations: {equations}
Variables: {variables}

Réponds en JSON strict:
{{
  "pseudocode": "pseudo-code textuel détaillé",
  "overall_complexity": "ex: O(n*d^2)",
  "memory_cost": "ex: O(d_model * n_mem)"
}}

Si les informations sont insuffisantes, mets le pseudo-code générique fourni et "INFORMATION NON DISPONIBLE DANS LE PAPIER" pour la complexité.""",
)


GITHUB_ANALYSIS_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Tu évalues la maturité d'un dépôt GitHub associé à une publication scientifique.

URL: {github_url}
Métadonnées: {repo_info}

Réponds en JSON strict:
{{
  "maturity_score": "0.0-1.0",
  "code_quality_score": "0.0-1.0",
  "reproducibility_score": "0.0-1.0",
  "documentation_score": "0.0-1.0",
  "strengths": ["..."],
  "weaknesses": ["..."],
  "risks": ["..."],
  "verdict": "prêt à l'emploi|nécessite du travail|insuffisant|non évaluable"
}}

Si aucune info n'est disponible, returns des scores de 0.0 et "non évaluable".""",
)


SCORING_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Évalue la publication suivante selon les critères d'intégration PAPERS.

Titre: {title}
Abstract: {abstract}
Contributions: {contributions}
Limites: {limitations}
Code disponible: {code_available}

Réponds en JSON strict:
{{
  "reproducibility": 0.0,
  "architectural_impact": 0.0,
  "hardware_cost": 0.0,
  "code_availability": 0.0,
  "scientific_maturity": 0.0,
  "justification": "texte justificatif court"
}}

Les valeurs doivent être des nombres flottants entre 0.0 et 1.0.""",
)


DEEP_ANALYSIS_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Effectue une analyse approfondie multi-passes de la publication.

Titre: {title}
Abstract: {abstract}
Texte: {text}

Réponds en JSON strict:
{{
  "pass_1_facts": ["faits extraits du papier"],
  "pass_2_methodology": "analyse de la méthode et de sa validité",
  "pass_3_limitations": ["limitations détectées"],
  "pass_4_integration_feasibility": "faisabilité d'intégration sur GPU 24 Go / RAM 64-256 Go",
  "pass_5_recommendation": "REJET|ARCHIVAGE|RECHERCHE SUPPLEMENTAIRE|PROTOTYPE|INTEGRATION",
  "confidence": "0.0-1.0",
  "key_open_questions": ["questions ouvertes critiques"]
}}

N'invente rien. Sois critique.""",
)


HTML_REPORT_PROMPT = LLMPrompt(
    system=SYSTEM_PAPERS_ANALYST,
    prompt="""Résume le rapport d'analyse suivant en HTML structuré et visuellement clair.

Données:
{report_json}

Réponds en JSON strict:
{{
  "html": "<!DOCTYPE html>..."
}}

Le HTML doit inclure: titre, scores, résumé exécutif, contributions, architecture, plan d'expérience, recommandation.""",
)


def format_prompt(template: str, **kwargs: str) -> str:
    return template.format(**{k: v for k, v in kwargs.items()})
