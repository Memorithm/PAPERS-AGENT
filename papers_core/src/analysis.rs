use regex::Regex;

use crate::engine::{AlgorithmDesc, AnalysisReport, Risk, SystemRequirements};
use crate::extraction::ExtractedDocument;

// ---------------------------------------------------------------------------
// Analyse mathématique
// ---------------------------------------------------------------------------

/// Résultat d'analyse mathématique heuristique.
#[derive(Debug, Clone)]
pub struct MathematicalAnalysis {
    pub equations: Vec<String>,
    pub variables: Vec<(String, String)>,
    pub loss_functions: Vec<String>,
    pub transformations: Vec<String>,
    pub uncertainties: Vec<String>,
}

pub struct MathematicalAnalyzer;

impl MathematicalAnalyzer {
    pub fn analyze(text: &str) -> MathematicalAnalysis {
        let eq_re = Regex::new(r"\$+[^$]+\$+").unwrap();
        let equations: Vec<String> = eq_re
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .take(50)
            .collect();

        let var_re = Regex::new(r"([A-Za-z][A-Za-z0-9_]*)\s*=\s*([^\n]+)").unwrap();
        let variables: Vec<(String, String)> = var_re
            .captures_iter(text)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .take(20)
            .collect();

        let loss_kws = [
            "loss", "perte", "objective", "coût", "coût", "L =", "\\mathcal{L}",
        ];
        let losses = Self::extract_sentences_by_keywords(text, &loss_kws, 10);

        let transform_kws = [
            "transformation", "mapping", "projection", "embedding", "encode", "decode",
        ];
        let transformations = Self::extract_sentences_by_keywords(text, &transform_kws, 10);

        MathematicalAnalysis {
            equations,
            variables,
            loss_functions: losses,
            transformations,
            uncertainties: Vec::new(),
        }
    }

    fn extract_sentences_by_keywords(text: &str, keywords: &[&str], limit: usize) -> Vec<String> {
        let sent_re = Regex::new(r"[.!?]\s+").unwrap();
        let lower = text.to_lowercase();
        let mut results = Vec::new();
        let mut last = 0;
        let mut splits: Vec<usize> = sent_re.find_iter(text).map(|m| m.start()).collect();
        splits.push(text.len());
        for end in splits {
            let sentence = &text[last..end];
            let sentence_lower = &lower[last..end];
            if keywords.iter().any(|k| sentence_lower.contains(k)) {
                results.push(sentence.trim().to_string());
                if results.len() >= limit {
                    break;
                }
            }
            last = end;
        }
        results
    }
}

// ---------------------------------------------------------------------------
// Analyse algorithmique
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AlgorithmStep {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct AlgorithmAnalysis {
    pub steps: Vec<AlgorithmStep>,
    pub pseudocode: String,
    pub overall_complexity: String,
    pub memory_cost: String,
}

pub struct AlgorithmAnalyzer;

impl AlgorithmAnalyzer {
    pub fn analyze(_text: &str) -> AlgorithmAnalysis {
        let steps = vec![
            AlgorithmStep {
                name: "Entrée".into(),
                description: "Capturer les entrées du système".into(),
            },
            AlgorithmStep {
                name: "Prétraitement".into(),
                description: "Normaliser et préparer les données".into(),
            },
            AlgorithmStep {
                name: "Calcul principal".into(),
                description: "Appliquer la transformation principale".into(),
            },
            AlgorithmStep {
                name: "Mise à jour état".into(),
                description: "Mettre à jour les états internes".into(),
            },
            AlgorithmStep {
                name: "Sortie".into(),
                description: "Produire le résultat".into(),
            },
        ];
        AlgorithmAnalysis {
            steps,
            pseudocode: "\nFONCTION ProcessPaper(input):\n    état = InitialiserÉtat()\n    données = Prétraiter(input)\n    POUR CHAQUE étape:\n        état = MettreÀJour(état, données)\n    RETOURNER ProduireSortie(état)\n".into(),
            overall_complexity: "INFORMATION NON DISPONIBLE DANS LE PAPIER".into(),
            memory_cost: "INFORMATION NON DISPONIBLE DANS LE PAPIER".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Analyse système (heuristique – retourne des valeurs par défaut)
// ---------------------------------------------------------------------------

pub struct SystemAnalyzer;

impl SystemAnalyzer {
    pub fn analyze(_text: &str) -> SystemRequirements {
        SystemRequirements {
            vram: None,
            ram: None,
            disk: None,
            latency: None,
            throughput: None,
            scalability: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Cartographie architecturale heuristique
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ArchitecturalMapping {
    pub perception: Vec<String>,
    pub memory: Vec<String>,
    pub planning: Vec<String>,
    pub decision: Vec<String>,
    pub action: Vec<String>,
    pub learning: Vec<String>,
    pub reflection: Vec<String>,
    pub evaluation: Vec<String>,
    pub impacted_modules: Vec<String>,
    pub required_interfaces: Vec<String>,
    pub dependencies: Vec<String>,
}

impl ArchitecturalMapping {
    pub fn empty() -> Self {
        Self {
            perception: Vec::new(),
            memory: Vec::new(),
            planning: Vec::new(),
            decision: Vec::new(),
            action: Vec::new(),
            learning: Vec::new(),
            reflection: Vec::new(),
            evaluation: Vec::new(),
            impacted_modules: Vec::new(),
            required_interfaces: Vec::new(),
            dependencies: Vec::new(),
        }
    }
}

pub struct HeuristicArchitecturalMapper;

impl HeuristicArchitecturalMapper {
    pub fn map(document: &ExtractedDocument, executive_summary: &str, contributions: &[String]) -> ArchitecturalMapping {
        let text = [
            document.title.clone(),
            document.abstract_text.clone().unwrap_or_default(),
            executive_summary.to_string(),
            contributions.join(" "),
            document.full_text.clone().unwrap_or_default(),
        ]
        .join(" ")
        .to_lowercase();

        let mut mapping = ArchitecturalMapping::empty();

        let keywords: Vec<(&str, Vec<&str>)> = vec![
            ("memory", vec!["memory", "mémoire", "kv cache", "long-term memory", "retrieval", "rag", "store", "recall"]),
            ("perception", vec!["perception", "embedding", "representation", "encoding", "encoder", "feature"]),
            ("planning", vec!["planning", "planification", "plan", "search", "mcts", "tree", "subgoal"]),
            ("decision", vec!["decision", "policy", "choix", "reasoning", "raisonnement", "inference"]),
            ("action", vec!["action", "tool use", "act", "execution", "environment", "agent"]),
            ("learning", vec!["learning", "apprentissage", "training", "fine-tuning", "distillation", "update"]),
            ("reflection", vec!["reflection", "reflect", "self-correct", "self-evaluation", "introspection"]),
            ("evaluation", vec!["evaluation", "eval", "benchmark", "metric", "reward", "score"]),
        ];

        for (pillar, kws) in &keywords {
            let matches: Vec<String> = kws
                .iter()
                .filter(|kw| text.contains(*kw))
                .map(|s| s.to_string())
                .collect();
            if !matches.is_empty() {
                match *pillar {
                    "memory" => mapping.memory.extend(matches),
                    "perception" => mapping.perception.extend(matches),
                    "planning" => mapping.planning.extend(matches),
                    "decision" => mapping.decision.extend(matches),
                    "action" => mapping.action.extend(matches),
                    "learning" => mapping.learning.extend(matches),
                    "reflection" => mapping.reflection.extend(matches),
                    "evaluation" => mapping.evaluation.extend(matches),
                    _ => {}
                }
            }
        }

        if !mapping.memory.is_empty() {
            mapping.impacted_modules.push("memory_module".into());
        }
        if !mapping.perception.is_empty() {
            mapping.impacted_modules.push("perception_module".into());
        }
        if !mapping.decision.is_empty() || !mapping.planning.is_empty() || !mapping.evaluation.is_empty() {
            mapping.impacted_modules.push("reasoning_module".into());
        }
        if !mapping.planning.is_empty() {
            mapping.impacted_modules.push("planning_module".into());
        }
        if !mapping.action.is_empty() {
            mapping.impacted_modules.push("action_module".into());
        }
        if !mapping.learning.is_empty() {
            mapping.impacted_modules.push("learning_module".into());
        }
        if !mapping.reflection.is_empty() {
            mapping.impacted_modules.push("reflection_module".into());
        }
        if !mapping.evaluation.is_empty() {
            mapping.impacted_modules.push("evaluation_module".into());
        }

        mapping.required_interfaces = mapping
            .impacted_modules
            .iter()
            .map(|m| format!("{}_api", m))
            .collect();
        mapping.dependencies = Self::detect_dependencies(&text);

        mapping
    }

    fn detect_dependencies(text: &str) -> Vec<String> {
        let mut deps = Vec::new();
        if text.contains("pytorch") || text.contains("torch") {
            deps.push("PyTorch".into());
        }
        if text.contains("jax") {
            deps.push("JAX".into());
        }
        if text.contains("tensorflow") {
            deps.push("TensorFlow".into());
        }
        if text.contains("cuda") {
            deps.push("CUDA".into());
        }
        if text.contains("rust") {
            deps.push("Rust".into());
        }
        if text.contains("c++") || text.contains("cpp") {
            deps.push("C++".into());
        }
        if text.contains("transformers") {
            deps.push("Hugging Face Transformers".into());
        }
        if text.contains("flash attention") {
            deps.push("Flash Attention".into());
        }
        deps
    }
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

pub struct ScoringEngine;

impl ScoringEngine {
    pub fn compute_reproducibility(document: &ExtractedDocument) -> f64 {
        let has_code = document
            .full_text
            .as_deref()
            .map(|t| t.contains("```") || t.contains("fn ") || t.contains("def ") || t.contains("class "))
            .unwrap_or(false);
        let has_full_text = document.full_text.as_deref().map(|t| t.len() > 10000).unwrap_or(false);
        let has_github = document.github_url.is_some();

        let documentation = if has_full_text { 0.8 } else { 0.5 };
        let code_available = if has_github { 0.5 } else { 0.0 };
        let code_examples = if has_code { 0.3 } else { 0.0 };

        0.4 * documentation + 0.3 * code_available + 0.3 * code_examples
    }

    pub fn compute_integration(document: &ExtractedDocument, reproducibility: f64, has_equations: bool, has_references: bool) -> f64 {
        let code_avail = if document.github_url.is_some() { 0.3 } else { 0.0 };
        let eq_score = if has_equations { 0.2 } else { 0.05 };
        let ref_score = if has_references { 0.1 } else { 0.0 };
        (reproducibility * 0.4 + code_avail + eq_score + ref_score).min(1.0)
    }
}

// ---------------------------------------------------------------------------
// Analyse des risques heuristique
// ---------------------------------------------------------------------------

pub struct RiskAnalyzer;

impl RiskAnalyzer {
    pub fn analyze(document: &ExtractedDocument, text: &str) -> Vec<Risk> {
        let mut risks = Vec::new();

        if document.github_url.is_none() {
            risks.push(Risk {
                level: "MEDIUM".into(),
                description: "Aucun code source n'est associé à la publication.".into(),
                mitigation: Some("Contacter les auteurs ou tenter une reproduction indépendante.".into()),
            });
        }

        if text.is_empty() {
            risks.push(Risk {
                level: "HIGH".into(),
                description: "Texte complet non disponible, l'analyse repose sur l'abstract.".into(),
                mitigation: Some("Récupérer le PDF complet pour une analyse approfondie.".into()),
            });
        }

        risks
    }
}

// ---------------------------------------------------------------------------
// Analyseur heuristique composite
// ---------------------------------------------------------------------------

pub struct HeuristicAnalyzer;

impl HeuristicAnalyzer {
    /// Exécute toutes les analyses heuristiques et retourne les résultats structurés.
    pub fn analyze(document: &ExtractedDocument, executive_summary: &str, contributions: &[String]) -> HeuristicAnalysisResult {
        let text = document.full_text.as_deref().unwrap_or("");
        let math = MathematicalAnalyzer::analyze(text);
        let algo = AlgorithmAnalyzer::analyze(text);
        let sys = SystemAnalyzer::analyze(text);
        let arch = HeuristicArchitecturalMapper::map(document, executive_summary, contributions);
        let reproducibility = ScoringEngine::compute_reproducibility(document);
        let integration = ScoringEngine::compute_integration(document, reproducibility, !math.equations.is_empty(), !document.references.is_empty());
        let risks = RiskAnalyzer::analyze(document, text);

        HeuristicAnalysisResult {
            math,
            algo,
            system: sys,
            architecture: arch,
            reproducibility,
            integration,
            risks,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeuristicAnalysisResult {
    pub math: MathematicalAnalysis,
    pub algo: AlgorithmAnalysis,
    pub system: SystemRequirements,
    pub architecture: ArchitecturalMapping,
    pub reproducibility: f64,
    pub integration: f64,
    pub risks: Vec<Risk>,
}

// ---------------------------------------------------------------------------
// Conversion vers AnalysisReport
// ---------------------------------------------------------------------------

impl HeuristicAnalysisResult {
    /// Enrichit un AnalysisReport avec les résultats d'analyse heuristique.
    pub fn enrich_report(&self, report: &mut AnalysisReport) {
        report.equations = self.math.equations.clone();
        report.variables = self.math.variables.clone();
        report.algorithms = vec![AlgorithmDesc {
            name: "Analyse heuristique".into(),
            complexity: Some(self.algo.overall_complexity.clone()),
            pseudocode: Some(self.algo.pseudocode.clone()),
        }];
        report.system_requirements = self.system.clone();
        report.risks = self.risks.clone();
        report.reproducibility_score = self.reproducibility;
        report.integration_score = self.integration;
        report.impacted_modules = self.architecture.impacted_modules.clone();
    }

    /// Ajoute les données de mapping architectural dans le rapport (sérialisé).
    pub fn architecture_json(&self) -> serde_json::Value {
        serde_json::json!({
            "perception": self.architecture.perception,
            "memory": self.architecture.memory,
            "planning": self.architecture.planning,
            "decision": self.architecture.decision,
            "action": self.architecture.action,
            "learning": self.architecture.learning,
            "reflection": self.architecture.reflection,
            "evaluation": self.architecture.evaluation,
            "impacted_modules": self.architecture.impacted_modules,
            "required_interfaces": self.architecture.required_interfaces,
            "dependencies": self.architecture.dependencies,
        })
    }

    pub fn extract_algo_descs(&self) -> Vec<AlgorithmDesc> {
        vec![AlgorithmDesc {
            name: "Pipeline heuristique".into(),
            complexity: Some(self.algo.overall_complexity.clone()),
            pseudocode: Some(self.algo.pseudocode.clone()),
        }]
    }
}
