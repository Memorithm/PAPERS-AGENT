//! Analyse LLM complète d'un document extrait.
//!
//! Porte `llm/analyzer.py` + `llm/prompts.py` du Python.
//! Chaque méthode appelle le LLM avec un prompt structuré, parse la réponse
//! JSON, et tombe en fallback heuristique si le LLM est indisponible.

use log::warn;
use std::cell::RefCell;

use crate::analysis::{
    AlgorithmAnalysis, HeuristicAnalysisResult, HeuristicArchitecturalMapper, MathematicalAnalysis,
};
use crate::engine::{AnalysisReport, Risk, SystemRequirements};
use crate::extraction::ExtractedDocument;
use crate::llm::LlmClient;

// ---------------------------------------------------------------------------
// Prompt templates (port de llm/prompts.py)
// ---------------------------------------------------------------------------

const SYSTEM_PAPERS_ANALYST: &str = "Tu es PAPERS, un agent de recherche appliquée spécialisé dans l'analyse de publications scientifiques pour architectures IA autonomes.\n\nRÈGLES ABSOLUES:\n- Ne jamais inventer une information.\n- Si une information est absente du texte, réponds \"INFORMATION NON DISPONIBLE DANS LE PAPIER\".\n- Toutes les affirmations doivent être fondées sur le texte fourni.\n- Réponds UNIQUEMENT en JSON valide, sans markdown, sans explications hors JSON.";

const CONTRIBUTIONS_PROMPT: &str = r#"Analyse le texte suivant d'une publication scientifique.

EXTRAIS les contributions scientifiques principales sous forme de liste.

Texte:
{text}

Réponds en JSON strict:
{{
  "contributions": ["contribution 1", "contribution 2", ...]
}}

Si aucune contribution claire n'est identifiable, returns {{"contributions": ["INFORMATION NON DISPONIBLE DANS LE PAPIER"]}}."#;

const EXECUTIVE_SUMMARY_PROMPT: &str = r#"Rédige un résumé exécutif de 3 paragraphes maximum pour la publication suivante.

Titre: {title}
Abstract: {abs}
Contributions: {contributions}

Réponds en JSON strict:
{{
  "executive_summary": "texte du résumé"
}}

Le résumé doit couvrir:
1. Objectif et hypothèse
2. Méthode clé et impact architecture agentique
3. Limites et niveau de confiance pour intégration"#;

const SYSTEM_ANALYSIS_PROMPT: &str = r#"Analyse le texte d'un papier et estime les coûts système sur une cible: GPU RTX 4090/5090 (24 Go), 64-256 Go RAM, NVMe, Linux.

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
}}"#;

const ARCHITECTURE_PROMPT: &str = r#"Cartographie la technique du papier sur l'architecture d'un agent IA autonome.

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

Utilise [] si un pilier n'est pas impacté."#;

const RISKS_PROMPT: &str = r#"Identifie les risques de la technique décrite dans le texte pour un système IA autonome en production locale.

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

Si aucun risque identifiable, returns {{"risks": []}}."#;

const MATHEMATICAL_PROMPT: &str = r#"Extrais les éléments mathématiques du texte.

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

N'invente pas d'équations. Utilise "INFORMATION NON DISPONIBLE DANS LE PAPIER" quand nécessaire."#;

const EXPERIMENT_PROMPT: &str = r#"Propose un plan d'expérimentation pour valider l'intégration de la technique dans une architecture IA autonome.

Titre: {title}
Abstract: {abs}
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
}}"#;

const PSEUDOCODE_PROMPT: &str = r#"Génère un pseudo-code détaillé pour implémenter la technique décrite.

Titre: {title}
Abstract: {abs}
Contributions: {contributions}
Équations: {equations}
Variables: {variables}

Réponds en JSON strict:
{{
  "pseudocode": "pseudo-code textuel détaillé",
  "overall_complexity": "ex: O(n*d^2)",
  "memory_cost": "ex: O(d_model * n_mem)"
}}

Si les informations sont insuffisantes, mets le pseudo-code générique fourni et "INFORMATION NON DISPONIBLE DANS LE PAPIER" pour la complexité."#;

const SCORING_PROMPT: &str = r#"Évalue la publication suivante selon les critères d'intégration PAPERS.

Titre: {title}
Abstract: {abs}
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

Les valeurs doivent être des nombres flottants entre 0.0 et 1.0."#;

const DEEP_ANALYSIS_PROMPT: &str = r#"Effectue une analyse approfondie multi-passes de la publication.

Titre: {title}
Abstract: {abs}
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

N'invente rien. Sois critique."#;

// ---------------------------------------------------------------------------
// LLM Analyzer
// ---------------------------------------------------------------------------

/// Analyseur LLM structuré, calqué sur le design de `LLMAnalyzer` Python.
///
/// Chaque méthode appelle le LLM local avec un prompt dédié puis parse la réponse JSON.
/// En cas d'échec LLM, un fallback heuristique est utilisé — sauf en mode strict
/// (`fallback: false`), où chaque échec est enregistré et remonté explicitement
/// via [`take_failures`](Self::take_failures) au lieu de passer inaperçu.
pub struct LLmAnalyzer<'a> {
    client: &'a LlmClient,
    fallback: bool,
    heuristic: HeuristicAnalysisResult,
    /// Échecs LLM (indisponibilité ou réponse non parsable), avec contexte.
    failures: RefCell<Vec<String>>,
}

impl<'a> LLmAnalyzer<'a> {
    pub fn new(client: &'a LlmClient) -> Self {
        Self {
            client,
            fallback: true,
            failures: RefCell::new(Vec::new()),
            heuristic: HeuristicAnalysisResult {
                math: MathematicalAnalysis {
                    equations: Vec::new(),
                    variables: Vec::new(),
                    loss_functions: Vec::new(),
                    transformations: Vec::new(),
                    uncertainties: Vec::new(),
                },
                algo: AlgorithmAnalysis {
                    steps: Vec::new(),
                    pseudocode: String::new(),
                    overall_complexity: "INFORMATION NON DISPONIBLE DANS LE PAPIER".into(),
                    memory_cost: "INFORMATION NON DISPONIBLE DANS LE PAPIER".into(),
                },
                system: SystemRequirements {
                    vram: None,
                    ram: None,
                    disk: None,
                    latency: None,
                    throughput: None,
                    scalability: None,
                },
                architecture: crate::analysis::ArchitecturalMapping::empty(),
                reproducibility: 0.0,
                integration: 0.0,
                risks: Vec::new(),
            },
        }
    }

    /// Initialise les résultats heuristiques pour fallback.
    pub fn with_heuristic(mut self, h: HeuristicAnalysisResult) -> Self {
        self.heuristic = h;
        self
    }

    /// Mode strict : désactive le fallback heuristique. Chaque échec LLM est
    /// enregistré (visible via [`take_failures`](Self::take_failures)) au lieu
    /// d'être masqué par des résultats heuristiques.
    pub fn strict(mut self) -> Self {
        self.fallback = false;
        self
    }

    /// Nombre d'échecs LLM enregistrés depuis la création.
    pub fn failure_count(&self) -> usize {
        self.failures.borrow().len()
    }

    /// Récupère et vide les échecs LLM enregistrés (contexte + cause).
    pub fn take_failures(&self) -> Vec<String> {
        std::mem::take(&mut *self.failures.borrow_mut())
    }

    // --- appels internes ---

    fn try_call_llm_json(&self, prompt: &str, system: &str) -> Result<serde_json::Value, String> {
        if !self.client.is_available() {
            return Err("LLM non disponible".into());
        }
        self.client.generate_json(prompt, Some(system))
    }

    /// Appelle le LLM ; en cas d'échec journalise le problème avec son contexte
    /// puis retourne Null. En mode strict l'échec est aussi enregistré pour
    /// remontée explicite dans le rapport final.
    fn call_llm_json(&self, prompt: &str, system: &str, context: &str) -> serde_json::Value {
        match self.try_call_llm_json(prompt, system) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("{context}: {e}");
                if self.fallback {
                    warn!("Erreur LLM ({msg}). Fallback heuristique.");
                } else {
                    warn!("Erreur LLM en mode strict ({msg}). Section marquée non disponible.");
                    self.failures.borrow_mut().push(msg);
                }
                serde_json::Value::Null
            }
        }
    }

    fn get_string(&self, obj: &serde_json::Value, key: &str, fallback: &str) -> String {
        obj.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(fallback)
            .to_string()
    }

    fn get_string_array(&self, obj: &serde_json::Value, key: &str) -> Vec<String> {
        obj.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_float(&self, obj: &serde_json::Value, key: &str, default: f64) -> f64 {
        obj.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
    }

    // --- API publique ---

    /// Analyse les contributions scientifiques.
    pub fn analyze_contributions(&self, report: &AnalysisReport) -> Vec<String> {
        let text = prepare_text(&report.document);
        let prompt = CONTRIBUTIONS_PROMPT.replace("{text}", &safe_truncate(&text, 8000));
        let result = self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "contributions");

        let contributions = self.get_string_array(&result, "contributions");
        if contributions.is_empty() && self.fallback {
            fallback_contributions(&text)
        } else if contributions.is_empty() {
            vec!["INFORMATION NON DISPONIBLE DANS LE PAPIER".into()]
        } else {
            contributions
        }
    }

    /// Résumé exécutif.
    pub fn analyze_executive_summary(
        &self,
        title: &str,
        abs: &str,
        contributions: &[String],
    ) -> String {
        let prompt = EXECUTIVE_SUMMARY_PROMPT
            .replace("{title}", title)
            .replace("{abs}", abs)
            .replace("{contributions}", &contributions.join("\n"));
        let result = self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "résumé exécutif");
        self.get_string(
            &result,
            "executive_summary",
            "INFORMATION NON DISPONIBLE DANS LE PAPIER",
        )
    }

    /// Analyse système (VRAM, RAM, etc.).
    pub fn analyze_system(&self, report: &AnalysisReport) -> SystemRequirements {
        let text = prepare_text(&report.document);
        let prompt = SYSTEM_ANALYSIS_PROMPT.replace("{text}", &safe_truncate(&text, 8000));
        let result = self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "exigences système");

        if result.is_null() {
            return self.heuristic.system.clone();
        }

        SystemRequirements {
            vram: Some(self.get_string(
                &result,
                "vram_consumption",
                "INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )),
            ram: Some(self.get_string(
                &result,
                "ram_consumption",
                "INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )),
            disk: Some(self.get_string(
                &result,
                "disk_io",
                "INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )),
            latency: Some(self.get_string(
                &result,
                "latency",
                "INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )),
            throughput: Some(self.get_string(
                &result,
                "throughput",
                "INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )),
            scalability: Some(self.get_string(
                &result,
                "scalability",
                "INFORMATION NON DISPONIBLE DANS LE PAPIER",
            )),
        }
    }

    /// Cartographie architecturale.
    pub fn analyze_architecture(
        &self,
        report: &AnalysisReport,
    ) -> crate::analysis::ArchitecturalMapping {
        let text = prepare_text(&report.document);
        let prompt = ARCHITECTURE_PROMPT.replace("{text}", &safe_truncate(&text, 8000));
        let result = self.call_llm_json(
            &prompt,
            SYSTEM_PAPERS_ANALYST,
            "cartographie architecturale",
        );

        if result.is_null() && self.fallback {
            return HeuristicArchitecturalMapper::map(
                &report.document,
                &report.executive_summary,
                &report.contributions,
            );
        }
        if result.is_null() {
            return crate::analysis::ArchitecturalMapping::empty();
        }

        crate::analysis::ArchitecturalMapping {
            perception: self.get_string_array(&result, "perception"),
            memory: self.get_string_array(&result, "memory"),
            planning: self.get_string_array(&result, "planning"),
            decision: self.get_string_array(&result, "decision"),
            action: self.get_string_array(&result, "action"),
            learning: self.get_string_array(&result, "learning"),
            reflection: self.get_string_array(&result, "reflection"),
            evaluation: self.get_string_array(&result, "evaluation"),
            impacted_modules: self.get_string_array(&result, "impacted_modules"),
            required_interfaces: self.get_string_array(&result, "required_interfaces"),
            dependencies: self.get_string_array(&result, "dependencies"),
        }
    }

    /// Analyse des risques.
    pub fn analyze_risks(&self, report: &AnalysisReport) -> Vec<Risk> {
        let text = prepare_text(&report.document);
        let prompt = RISKS_PROMPT.replace("{text}", &safe_truncate(&text, 8000));
        let result = self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "risques");

        // Parse le tableau [{level, description, mitigation}]
        let risks_from_json = result
            .get("risks")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let level = r.get("level").and_then(|v| v.as_str()).unwrap_or("MEDIUM");
                        let description =
                            r.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        let mitigation = r.get("mitigation").and_then(|v| v.as_str());
                        if description.is_empty() {
                            None
                        } else {
                            Some(Risk {
                                level: level.to_string(),
                                description: description.to_string(),
                                mitigation: mitigation.map(|s| s.to_string()),
                            })
                        }
                    })
                    .collect::<Vec<Risk>>()
            })
            .unwrap_or_default();

        if risks_from_json.is_empty() && self.fallback {
            crate::analysis::RiskAnalyzer::analyze(&report.document, &text)
        } else if risks_from_json.is_empty() {
            Vec::new()
        } else {
            risks_from_json
        }
    }

    /// Analyse mathématique.
    pub fn analyze_mathematical(&self, text: &str) -> MathematicalAnalysis {
        let prompt = MATHEMATICAL_PROMPT.replace("{text}", &safe_truncate(text, 8000));
        let result = self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "analyse mathématique");

        if result.is_null() {
            return self.heuristic.math.clone();
        }

        variables_from_json(&result)
    }

    /// Plan d'expérimentation.
    pub fn analyze_experiment(
        &self,
        title: &str,
        abs: &str,
        contributions: &[String],
    ) -> serde_json::Value {
        let prompt = EXPERIMENT_PROMPT
            .replace("{title}", title)
            .replace("{abs}", abs)
            .replace("{contributions}", &contributions.join("\n"));
        self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "plan d'expérimentation")
    }

    /// Pseudo-code.
    pub fn analyze_pseudocode(
        &self,
        title: &str,
        abs: &str,
        contributions: &[String],
        math: &MathematicalAnalysis,
    ) -> serde_json::Value {
        let equations = math.equations.join("\n");
        let variables: String = math
            .variables
            .iter()
            .map(|(n, m)| format!("{}: {}", n, m))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = PSEUDOCODE_PROMPT
            .replace("{title}", title)
            .replace("{abs}", abs)
            .replace("{contributions}", &contributions.join("\n"))
            .replace("{equations}", &equations)
            .replace("{variables}", &variables);
        self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "pseudo-code")
    }

    /// Scoring LLM (reproductibilité + intégration).
    pub fn analyze_scoring(
        &self,
        title: &str,
        abs: &str,
        contributions: &[String],
        limitations: &[String],
        code_available: bool,
    ) -> Option<(f64, f64)> {
        let prompt = SCORING_PROMPT
            .replace("{title}", title)
            .replace("{abs}", abs)
            .replace("{contributions}", &contributions.join("\n"))
            .replace("{limitations}", &limitations.join("\n"))
            .replace(
                "{code_available}",
                if code_available { "oui" } else { "non" },
            );
        let result = self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "scoring");

        if result.is_null() {
            return None;
        }

        Some((
            self.get_float(&result, "reproducibility", 0.5),
            self.get_float(&result, "architectural_impact", 0.5),
        ))
    }

    /// Analyse approfondie multi-passes.
    pub fn analyze_deep(&self, title: &str, abs: &str, text: &str) -> serde_json::Value {
        let full_text = if text.len() > 12000 {
            safe_truncate(text, 12000)
        } else {
            text.to_string()
        };
        let prompt = DEEP_ANALYSIS_PROMPT
            .replace("{title}", title)
            .replace("{abs}", abs)
            .replace("{text}", &full_text);
        self.call_llm_json(&prompt, SYSTEM_PAPERS_ANALYST, "analyse approfondie")
    }
}

// ---------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------

fn prepare_text(document: &ExtractedDocument) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        document.title,
        document.abstract_text.as_deref().unwrap_or(""),
        document.full_text.as_deref().unwrap_or("")
    )
}

fn fallback_contributions(text: &str) -> Vec<String> {
    let keywords = [
        "propose",
        "introduce",
        "contribution",
        "we show",
        "we demonstrate",
        "we present",
    ];
    let mut results = Vec::new();
    for sentence in text.split(['.', '!', '?']) {
        let s = sentence.trim();
        if !s.is_empty() && s.len() > 20 && keywords.iter().any(|k| s.to_lowercase().contains(k)) {
            results.push(s.to_string());
            if results.len() >= 5 {
                break;
            }
        }
    }
    results
}

fn variables_from_json(result: &serde_json::Value) -> MathematicalAnalysis {
    let variables: Vec<(String, String)> = result
        .get("variables")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let name = v.get("name").and_then(|n| n.as_str())?.to_string();
                    let meaning = v
                        .get("meaning")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some((name, meaning))
                })
                .collect()
        })
        .unwrap_or_default();

    MathematicalAnalysis {
        equations: normalize_strings(result.get("equations")),
        variables,
        loss_functions: normalize_strings(result.get("loss_functions")),
        transformations: normalize_strings(result.get("transformations")),
        uncertainties: normalize_strings(result.get("uncertainties")),
    }
}

fn normalize_strings(val: Option<&serde_json::Value>) -> Vec<String> {
    match val {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Object(obj) => obj
                    .iter()
                    .filter_map(|(_, v)| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
                    .join(" - "),
                other => other.to_string(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Safe UTF-8-aware string truncation. Never panics on multi-byte boundaries.
fn safe_truncate(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::LlmConfig;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// Serveur HTTP mock (même principe que celui de llm.rs).
    fn spawn_mock(responses: Vec<(u16, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                // Lit les en-têtes ET le corps de la requête : fermer le socket
                // avec des données non lues déclencherait un RST côté client.
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let mut content_length = 0usize;
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                }
                if content_length > 0 {
                    let mut sink = vec![0u8; content_length];
                    let _ = reader.read_exact(&mut sink);
                }
                let code = match status {
                    200 => "200 OK",
                    _ => "500 Internal Server Error",
                };
                let payload = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    code,
                    body.len(),
                    body
                );
                let _ = stream.write_all(payload.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(std::net::Shutdown::Write);
            }
        });
        addr
    }

    fn client_at(addr: &str) -> LlmClient {
        LlmClient::new(LlmConfig {
            provider: "openai".into(),
            base_url: format!("http://{}", addr),
            api_key: None,
            timeout_secs: 5,
            retry_attempts: 0,
            retry_backoff_ms: 1,
            ..LlmConfig::default()
        })
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            document: ExtractedDocument {
                id: "T-1".into(),
                title: "Test Paper".into(),
                authors: vec![],
                publication_date: None,
                source: "test".into(),
                paper_url: None,
                github_url: None,
                abstract_text: Some("We propose a novel transformer.".into()),
                full_text: Some(
                    "We propose a novel method. Contribution: faster inference.".into(),
                ),
                references: vec![],
                parsed: None,
            },
            contributions: Vec::new(),
            executive_summary: String::new(),
            equations: Vec::new(),
            variables: Vec::new(),
            algorithms: Vec::new(),
            system_requirements: SystemRequirements {
                vram: None,
                ram: None,
                disk: None,
                latency: None,
                throughput: None,
                scalability: None,
            },
            risks: Vec::new(),
            recommendation: crate::engine::Recommendation::Archive,
            recommendation_justification: String::new(),
            integration_score: 0.5,
            reproducibility_score: 0.5,
            impacted_modules: Vec::new(),
            timestamp: String::new(),
            architectural_mapping: serde_json::Value::Null,
            deep_analysis: serde_json::Value::Null,
            experiment_plan: serde_json::Value::Null,
            pseudo_code: serde_json::Value::Null,
            llm_warnings: Vec::new(),
        }
    }

    #[test]
    fn strict_mode_records_failure_when_llm_returns_garbage() {
        let addr = spawn_mock(vec![(200, "ceci n'est pas du json")]);
        let client = client_at(&addr);
        let analyzer = LLmAnalyzer::new(&client).strict();
        let report = sample_report();

        let risks = analyzer.analyze_risks(&report);
        assert!(
            risks.is_empty(),
            "mode strict : pas de fallback heuristique"
        );
        assert_eq!(analyzer.failure_count(), 1);

        let failures = analyzer.take_failures();
        assert!(
            failures[0].contains("risques"),
            "contexte attendu: {}",
            failures[0]
        );
        assert_eq!(analyzer.failure_count(), 0, "take_failures vide la liste");
    }

    #[test]
    fn fallback_mode_hides_failure_but_does_not_record_it() {
        let addr = spawn_mock(vec![(200, "toujours pas de json")]);
        let client = client_at(&addr);
        let analyzer = LLmAnalyzer::new(&client); // fallback actif
        let report = sample_report();

        let contributions = analyzer.analyze_contributions(&report);
        assert!(!contributions.is_empty(), "fallback heuristique appliqué");
        assert_eq!(
            analyzer.failure_count(),
            0,
            "fallback : aucun échec enregistré"
        );
    }

    #[test]
    fn successful_llm_response_produces_no_failures() {
        let inner =
            r#"{"risks":[{"level":"HIGH","description":"dérive","mitigation":"garde-fous"}]}"#;
        let envelope = format!(
            r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#,
            serde_json::to_string(inner).unwrap()
        );
        let addr = spawn_mock(vec![(200, Box::leak(envelope.into_boxed_str()))]);
        let client = client_at(&addr);
        let analyzer = LLmAnalyzer::new(&client).strict();
        let report = sample_report();

        let risks = analyzer.analyze_risks(&report);
        assert_eq!(risks.len(), 1);
        assert_eq!(risks[0].level, "HIGH");
        assert_eq!(analyzer.failure_count(), 0);
    }
}
