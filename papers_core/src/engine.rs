use std::collections::HashMap;

use log::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::doc_store::{DocStore, SearchResult};
use crate::evolution::EvolutionLoop;
use crate::extraction::{ExtractedDocument, ExtractionPipeline};
use crate::llm::{LlmClient, LlmConfig};
use crate::models::{CognitionItem, EvolutionConfig, EvolutionResult};
use crate::paper_parser::PaperParser;

/// Rapport d'analyse complet d'un papier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub document: ExtractedDocument,
    pub contributions: Vec<String>,
    pub executive_summary: String,
    pub equations: Vec<String>,
    pub variables: Vec<(String, String)>,
    pub algorithms: Vec<AlgorithmDesc>,
    pub system_requirements: SystemRequirements,
    pub risks: Vec<Risk>,
    pub recommendation: Recommendation,
    pub recommendation_justification: String,
    pub integration_score: f64,
    pub reproducibility_score: f64,
    pub impacted_modules: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmDesc {
    pub name: String,
    pub complexity: Option<String>,
    pub pseudocode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRequirements {
    pub vram: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
    pub latency: Option<String>,
    pub throughput: Option<String>,
    pub scalability: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    pub level: String,
    pub description: String,
    pub mitigation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Recommendation {
    Reject,
    Archive,
    Prototype,
    Integrate,
}

impl Recommendation {
    pub fn label(&self) -> &str {
        match self {
            Self::Reject => "REJET",
            Self::Archive => "ARCHIVAGE",
            Self::Prototype => "PROTOTYPE",
            Self::Integrate => "INTEGRATION",
        }
    }

    pub fn from_score(score: f64) -> Self {
        if score <= 0.30 { Self::Reject }
        else if score <= 0.50 { Self::Archive }
        else if score <= 0.70 { Self::Prototype }
        else { Self::Integrate }
    }
}

/// Moteur principal PAPERS V2 : extraction → analyse → évolution.
pub struct PapersEngine {
    pub llm: Option<LlmClient>,
    pub pipeline: ExtractionPipeline,
    pub doc_store: DocStore,
}

impl PapersEngine {
    /// Crée un moteur avec ou sans LLM.
    pub fn new(use_llm: bool, llm_config: Option<LlmConfig>) -> Self {
        let corpus = &[
            "reinforcement learning neural networks",
            "graph neural networks knowledge graphs",
            "symbolic regression genetic programming",
            "natural language processing transformers",
            "computer vision convolutional networks",
            "autonomous agents multi-agent systems",
            "recursive self-improvement meta-learning",
            "mechanistic interpretability neural networks",
            "quantum machine learning tensor networks",
            "AI safety alignment robustness",
        ];

        let llm = if use_llm {
            let config = llm_config.unwrap_or_default();
            let client = LlmClient::new(config.clone());
            if client.is_available() {
                info!("LLM connecté: {} @ {}", config.provider, config.model);
                Some(client)
            } else {
                warn!("LLM non disponible, analyse heuristique uniquement");
                None
            }
        } else {
            None
        };

        Self {
            llm,
            pipeline: ExtractionPipeline::new(),
            doc_store: DocStore::new(corpus),
        }
    }

    /// Extrait un document depuis une source (PDF, arXiv, URL, texte).
    pub fn extract(&mut self, source: &str) -> Result<ExtractedDocument, String> {
        info!("Extraction depuis: {}", source);
        let doc = self.pipeline.extract(source)?;

        // Indexer dans le DocStore
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::json!(source));
        metadata.insert("title".to_string(), serde_json::json!(&doc.title));
        if let Some(ref dt) = doc.publication_date {
            metadata.insert("date".to_string(), serde_json::json!(dt));
        }
        self.doc_store.add(&doc.id, &doc.title, metadata);

        Ok(doc)
    }

    /// Analyse un document extrait (avec ou sans LLM).
    pub fn analyze(&mut self, document: &ExtractedDocument) -> AnalysisReport {
        info!("Analyse de: {}", document.title);

        let text = document.full_text.as_deref().unwrap_or("");
        let abstract_text = document.abstract_text.as_deref().unwrap_or("");

        // Extraction heuristique de base
        let parsed = document.parsed.as_ref().cloned().unwrap_or_else(|| {
            PaperParser::parse(text, Some(&document.title))
        });

        // Analyse LLM si disponible
        let (summary, contributions) = if let Some(ref llm) = self.llm {
            let summary_prompt = format!(
                "Analyse ce papier scientifique et fournis un résumé exécutif concis en français (max 5 phrases).\n\
                 Titre: {}\nAuteurs: {}\nAbstract: {}\nTexte: {}...",
                document.title,
                document.authors.join(", "),
                abstract_text,
                &text[..text.len().min(4000)]
            );
            let summary = llm.generate(&summary_prompt, Some(
                "Tu es un analyste scientifique. Réponds en français de façon concise.",
            )).unwrap_or_else(|_| "Analyse LLM non disponible.".into());

            let contrib_prompt = format!(
                "Liste les 3 à 5 contributions scientifiques principales de ce papier. \
                 Format: une contribution par ligne, commence chaque ligne par '- '.\n\
                 Titre: {}\nTexte: {}...",
                document.title,
                &text[..text.len().min(3000)]
            );
            let contrib_text = llm.generate(&contrib_prompt, Some(
                "Liste uniquement les contributions, une par ligne, en français."
            )).unwrap_or_default();
            let contributions: Vec<String> = contrib_text
                .lines()
                .filter(|l| l.trim().starts_with('-') || l.trim().starts_with('•'))
                .map(|l| l.trim().trim_start_matches('-').trim_start_matches('•').trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();

            (summary, contributions)
        } else {
            let summary = format!(
                "Le papier '{}' (source: {}) est analysé de manière heuristique. \
                 Cette analyse préliminaire nécessite un examen humain complémentaire.",
                document.title, document.source
            );
            let contributions = if let Some(ref abs) = document.abstract_text {
                vec![abs.clone()]
            } else if !text.is_empty() {
                vec![text[..text.len().min(500)].to_string()]
            } else {
                vec!["Contribution à extraire manuellement.".into()]
            };
            (summary, contributions)
        };

        // Scoring
        let has_github = document.github_url.is_some();
        let has_code = text.contains("```") || text.contains("def ") || text.contains("fn ") ||
            text.contains("class ");
        let has_equations = !parsed.equations.is_empty();
        let has_references = !document.references.is_empty();

        let reproducibility = if has_code && has_github { 0.8 }
            else if has_code { 0.5 }
            else if has_github { 0.4 }
            else { 0.2 };
        let integration = (reproducibility * 0.5_f64
            + if has_equations { 0.3_f64 } else { 0.1_f64 }
            + if has_references { 0.2_f64 } else { 0.0_f64 })
            .min(1.0);

        let recommendation = Recommendation::from_score(integration);

        AnalysisReport {
            equations: parsed.equations,
            variables: parsed.variables.iter().map(|v| (v.name.clone(), v.meaning.clone())).collect(),
            algorithms: Vec::new(),
            system_requirements: SystemRequirements {
                vram: None, ram: None, disk: None, latency: None, throughput: None, scalability: None,
            },
            risks: Vec::new(),
            recommendation,
            recommendation_justification: format!(
                "Score d'intégration: {:.2}. Reproductibilité: {:.2}. {}",
                integration, reproducibility,
                if has_github { "Code disponible." } else { "Code non détecté." }
            ),
            integration_score: integration,
            reproducibility_score: reproducibility,
            impacted_modules: Vec::new(),
            contributions,
            executive_summary: summary,
            document: document.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Recherche des papiers similaires dans le DocStore.
    pub fn find_similar(&mut self, query: &str, top_k: usize) -> Vec<SearchResult> {
        self.doc_store.search(query, top_k)
    }

    /// Lance l'évolution sur un document extrait, en utilisant le LLM pour
    /// générer du code basé sur les concepts du papier.
    pub fn evolve(
        &mut self,
        document: &ExtractedDocument,
        config: EvolutionConfig,
    ) -> Result<EvolutionResult, String> {
        let llm = self.llm.as_ref().ok_or("LLM requis pour l'évolution")?;

        info!("Démarrage évolution: {}", config.task_description);

        let mut evo = EvolutionLoop::new(config.clone());

        // Seeder la cognition avec le contenu du papier
        let abstracts = document.abstract_text.clone().unwrap_or_default();
        let full = document.full_text.clone().unwrap_or_default();
        evo.seed_cognition(vec![
            CognitionItem::new(
                format!("Papier: {}", document.title),
                document.source.clone(),
                vec!["paper".into(), "source".into()],
            ),
            CognitionItem::new(
                abstracts.chars().take(2000).collect(),
                document.id.clone(),
                vec!["abstract".into()],
            ),
            CognitionItem::new(
                full.chars().take(4000).collect(),
                document.id.clone(),
                vec!["full_text".into()],
            ),
        ]);

        let task = config.task_description.clone();
        let _candidate_count = config.n_candidates_per_round;

        let result = evo.run(|query| {
            let prompt = format!(
                "Tu es un ingénieur Rust. GÉNÈRE UNIQUEMENT du code Rust valide.\n\n\
                 TÂCHE: {}\n\n\
                 CONTEXTE DU PAPIER: {}\n\n\
                 CONTEXTE D'ÉVOLUTION: {}\n\n\
                 FORMAT DE SORTIE STRICT:\n\
                 ```rust\n\
                 // Ton code ici - fonction complète et exécutable\n\
                 ```\n\n\
                 RÈGLES:\n\
                 - Code Rust UNIQUEMENT\n\
                 - Pas de commentaires hors code\n\
                 - Pas d'explications avant/après\n\
                 - La fonction doit être auto-suffisante",
                task,
                &document.title,
                query
            );

            match llm.generate(&prompt, Some(
                "Tu es un expert Rust. Génère uniquement du code. Pas d'explications."
            )) {
                Ok(code) => {
                    // Vérification basique du code
                    let has_rust = code.contains("fn ") || code.contains("struct ") ||
                        code.contains("impl ") || code.contains("use ");
                    let has_braces = code.contains('{') && code.contains('}');
                    let is_valid = has_rust && has_braces;
                    let score = if is_valid {
                        // Score basé sur la complexité du code
                        let lines = code.lines().count() as f64;
                        let complexity = lines.min(50.0) / 50.0;
                        0.3 + complexity * 0.5
                    } else {
                        0.1
                    };
                    (is_valid, score)
                }
                Err(e) => {
                    error!("LLM error: {}", e);
                    (false, 0.0)
                }
            }
        });

        info!(
            "Évolution terminée: best={:.4}, {} candidats, {:.1}s",
            result.best_score, result.total_candidates, result.total_time_secs
        );

        Ok(result)
    }
}

/// Pipeline complet: extraire → analyser → évoluer.
pub struct PipelineResult {
    pub document: ExtractedDocument,
    pub analysis: AnalysisReport,
    pub evolution: Option<EvolutionResult>,
    pub duration_secs: f64,
}

impl PapersEngine {
    /// Exécute le pipeline complet sur une source.
    pub fn run_pipeline(
        &mut self,
        source: &str,
        evolve: bool,
        evolution_config: Option<EvolutionConfig>,
    ) -> Result<PipelineResult, String> {
        let start = std::time::Instant::now();

        // 1. Extraction
        let document = self.extract(source)?;

        // 2. Analyse
        let analysis = self.analyze(&document);

        // 3. Évolution (optionnelle)
        let evolution = if evolve {
            let config = evolution_config.unwrap_or_else(|| EvolutionConfig {
                task_description: format!(
                    "Implémente en Rust les concepts du papier: {}",
                    document.title
                ),
                ..Default::default()
            });
            Some(self.evolve(&document, config)?)
        } else {
            None
        };

        let duration = start.elapsed().as_secs_f64();

        Ok(PipelineResult {
            document,
            analysis,
            evolution,
            duration_secs: duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = PapersEngine::new(false, None);
        assert!(engine.llm.is_none());
        // DocStore est créé mais vide (pas de documents ajoutés)
        assert!(engine.doc_store.is_empty());
    }

    #[test]
    fn test_recommendation_from_score() {
        assert_eq!(Recommendation::from_score(0.2), Recommendation::Reject);
        assert_eq!(Recommendation::from_score(0.4), Recommendation::Archive);
        assert_eq!(Recommendation::from_score(0.6), Recommendation::Prototype);
        assert_eq!(Recommendation::from_score(0.9), Recommendation::Integrate);
    }
}
