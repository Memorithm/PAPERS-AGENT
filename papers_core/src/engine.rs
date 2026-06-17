use std::collections::HashMap;

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::analysis::HeuristicAnalyzer;
use crate::doc_store::{DocStore, SearchResult};
use crate::evolution::EvolutionLoop;
use crate::extraction::{ExtractedDocument, ExtractionPipeline};
use crate::llm::{LlmClient, LlmConfig};
use crate::llm_analyzer::LLmAnalyzer;
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
    pub architectural_mapping: serde_json::Value,
    pub deep_analysis: serde_json::Value,
    pub experiment_plan: serde_json::Value,
    pub pseudo_code: serde_json::Value,
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
    ///
    /// Utilise l'analyseur heuristique (`HeuristicAnalyzer`) pour l'extraction
    /// rapide (équations, variables, risques, scoring) et, si un LLM est
    /// disponible, le `LLmAnalyzer` pour l'analyse approfondie (contributions,
    /// résumé, architecture, expérience, pseudo-code, analyse multi-passes).
    pub fn analyze(&mut self, document: &ExtractedDocument) -> AnalysisReport {
        info!("Analyse de: {}", document.title);

        let text = document.full_text.as_deref().unwrap_or("");
        let abstract_text = document.abstract_text.as_deref().unwrap_or("");

        // Extraction heuristique de base
        let parsed = document.parsed.as_ref().cloned().unwrap_or_else(|| {
            PaperParser::parse(text, Some(&document.title))
        });

        // Analyse LLM si disponible
        let (summary, contributions, arch, deep, exp, pseudo) = if let Some(ref llm) = self.llm {
            let analyzer = LLmAnalyzer::new(llm);

            let contributions = analyzer.analyze_contributions(&AnalysisReport {
                document: document.clone(),
                ..Self::empty_report(document, &parsed)
            });
            let summary = analyzer.analyze_executive_summary(
                &document.title,
                abstract_text,
                &contributions,
            );

            // Analyses LLM enrichies
            let arch = analyzer.analyze_architecture(&AnalysisReport {
                document: document.clone(),
                executive_summary: summary.clone(),
                contributions: contributions.clone(),
                ..Self::empty_report(document, &parsed)
            });
            let deep = analyzer.analyze_deep(&document.title, abstract_text, text);
            let exp = analyzer.analyze_experiment(&document.title, abstract_text, &contributions);
            let math = analyzer.analyze_mathematical(text);
            let pseudo = analyzer.analyze_pseudocode(
                &document.title, abstract_text, &contributions, &math,
            );

            (summary, contributions, arch, deep, exp, pseudo)
        } else {
            let summary = format!(
                "Analyse heuristique du papier '{}' (source: {}). \
                 Un LLM est nécessaire pour une analyse approfondie.",
                document.title, document.source
            );
            let contributions = if let Some(ref abs) = document.abstract_text {
                vec![abs.clone()]
            } else if !text.is_empty() {
                vec![text[..text.len().min(500)].to_string()]
            } else {
                vec!["Contribution à extraire manuellement.".into()]
            };
            let heuristic = HeuristicAnalyzer::analyze(document, &summary, &contributions);

            let arch = heuristic.architecture.clone();
            let deep = serde_json::Value::Null;
            let exp = serde_json::Value::Null;
            let pseudo = serde_json::Value::Null;

            (summary, contributions, arch, deep, exp, pseudo)
        };

        // Recalculer les scores avec le ScoringEngine
        let heuristic = HeuristicAnalyzer::analyze(document, &summary, &contributions);
        let reproducibility = heuristic.reproducibility;
        let integration = heuristic.integration;
        let recommendation = Recommendation::from_score(integration);

        AnalysisReport {
            equations: parsed.equations.clone(),
            variables: parsed.variables.iter().map(|v| (v.name.clone(), v.meaning.clone())).collect(),
            algorithms: heuristic.extract_algo_descs(),
            system_requirements: heuristic.system.clone(),
            risks: heuristic.risks.clone(),
            recommendation,
            recommendation_justification: format!(
                "Score d'intégration: {:.2}. Reproductibilité: {:.2}. Modules impactés: {}.",
                integration,
                reproducibility,
                heuristic.architecture.impacted_modules.join(", ")
            ),
            integration_score: integration,
            reproducibility_score: reproducibility,
            impacted_modules: heuristic.architecture.impacted_modules.clone(),
            contributions,
            executive_summary: summary,
            document: document.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            architectural_mapping: serde_json::json!({
                "perception": arch.perception,
                "memory": arch.memory,
                "planning": arch.planning,
                "decision": arch.decision,
                "action": arch.action,
                "learning": arch.learning,
                "reflection": arch.reflection,
                "evaluation": arch.evaluation,
                "impacted_modules": arch.impacted_modules,
                "required_interfaces": arch.required_interfaces,
                "dependencies": arch.dependencies,
            }),
            deep_analysis: deep,
            experiment_plan: exp,
            pseudo_code: pseudo,
        }
    }

    /// Rapport vide avec les champs obligatoires remplis (utile pour les
    /// appels intermédiaires à l'analyseur LLM).
    fn empty_report(document: &ExtractedDocument, parsed: &crate::paper_parser::ParsedPaper) -> AnalysisReport {
        AnalysisReport {
            document: document.clone(),
            contributions: Vec::new(),
            executive_summary: String::new(),
            equations: parsed.equations.clone(),
            variables: parsed.variables.iter().map(|v| (v.name.clone(), v.meaning.clone())).collect(),
            algorithms: Vec::new(),
            system_requirements: SystemRequirements {
                vram: None, ram: None, disk: None, latency: None, throughput: None, scalability: None,
            },
            risks: Vec::new(),
            recommendation: Recommendation::Reject,
            recommendation_justification: String::new(),
            integration_score: 0.0,
            reproducibility_score: 0.0,
            impacted_modules: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            architectural_mapping: serde_json::Value::Null,
            deep_analysis: serde_json::Value::Null,
            experiment_plan: serde_json::Value::Null,
            pseudo_code: serde_json::Value::Null,
        }
    }

    /// Recherche des papiers similaires dans le DocStore.
    pub fn find_similar(&mut self, query: &str, top_k: usize) -> Vec<SearchResult> {
        self.doc_store.search(query, top_k)
    }

    /// Lance l'évolution sur un document extrait, en utilisant le LLM
    /// avec le pipeline Researcher → Engineer.
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

        let result = evo.run_advanced(llm, &task);

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
