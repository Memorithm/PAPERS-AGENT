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
            "loss",
            "perte",
            "objective",
            "coût",
            "coût",
            "L =",
            "\\mathcal{L}",
        ];
        let losses = Self::extract_sentences_by_keywords(text, &loss_kws, 10);

        let transform_kws = [
            "transformation",
            "mapping",
            "projection",
            "embedding",
            "encode",
            "decode",
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
    pub fn analyze(text: &str) -> AlgorithmAnalysis {
        let steps = Self::extract_steps(text);
        let pseudocode = Self::extract_pseudocode(text);
        let overall_complexity = Self::extract_complexity(text);
        let memory_cost = Self::extract_memory_cost(text);

        AlgorithmAnalysis {
            steps,
            pseudocode,
            overall_complexity,
            memory_cost,
        }
    }

    fn extract_steps(text: &str) -> Vec<AlgorithmStep> {
        let lower = text.to_lowercase();
        let mut steps = Vec::new();

        let step_patterns = [
            (
                r"(?:first|initially|start)\b.{20,200}?(?:\.|;)",
                "Initialisation",
            ),
            (
                r"(?:then|next|subsequently|after\s+that)\b.{20,200}?(?:\.|;)",
                "Étape principale",
            ),
            (
                r"(?:finally|lastly|in\s+the\s+end)\b.{20,200}?(?:\.|;)",
                "Finalisation",
            ),
            (
                r"(?:step\s+\d+|phase\s+\d+|stage\s+\d+)\s*[:\-–—]?\s*.{20,200}?(?:\.|;)",
                "Étape",
            ),
            (
                r"(?:input|preprocess|normalize|tokenize)\b.{20,200}?(?:\.|;)",
                "Prétraitement",
            ),
            (
                r"(?:compute|calculate|estimate|evaluate|optimize)\b.{20,200}?(?:\.|;)",
                "Calcul",
            ),
            (
                r"(?:output|return|produce|generate|emit)\b.{20,200}?(?:\.|;)",
                "Sortie",
            ),
            (
                r"(?:update|iterate|loop|repeat|converge)\b.{20,200}?(?:\.|;)",
                "Mise à jour",
            ),
        ];

        for (pat, label) in &step_patterns {
            if let Ok(re) = regex::Regex::new(pat) {
                for cap in re.find_iter(&lower).take(2) {
                    let desc = cap.as_str().trim().chars().take(150).collect::<String>();
                    steps.push(AlgorithmStep {
                        name: label.to_string(),
                        description: desc,
                    });
                }
            }
        }

        // If no steps found, provide generic pipeline
        if steps.is_empty() {
            steps = vec![
                AlgorithmStep {
                    name: "Entrée".into(),
                    description: "Capturer et normaliser les données d'entrée".into(),
                },
                AlgorithmStep {
                    name: "Prétraitement".into(),
                    description: "Transformer les données pour le calcul".into(),
                },
                AlgorithmStep {
                    name: "Traitement principal".into(),
                    description: "Appliquer l'algorithme ou le modèle".into(),
                },
                AlgorithmStep {
                    name: "Post-traitement".into(),
                    description: "Interpréter les résultats".into(),
                },
                AlgorithmStep {
                    name: "Sortie".into(),
                    description: "Produire le résultat final".into(),
                },
            ];
        }

        steps
    }

    fn extract_pseudocode(text: &str) -> String {
        // Look for code blocks or algorithmic descriptions
        if let Ok(re_code) = regex::Regex::new(r"```[\s\S]*?```") {
            if let Some(cap) = re_code.find(text) {
                return cap.as_str().to_string();
            }
        }

        // Extract algorithmic phrases
        let lower = text.to_lowercase();
        let algo_kw = [
            "algorithm",
            "pseudocode",
            "procédure",
            "procedure",
            "function",
        ];
        let mut lines = Vec::new();

        if let Ok(re_sent) = regex::Regex::new(r"[.!?]\s+") {
            let mut last = 0;
            let mut splits: Vec<usize> = re_sent.find_iter(text).map(|m| m.start()).collect();
            splits.push(text.len());
            for end in splits {
                let sentence = &text[last..end];
                let s_lower = &lower[last..end];
                if algo_kw.iter().any(|k| s_lower.contains(k)) {
                    lines.push(format!("  // {}", sentence.trim()));
                    if lines.len() >= 6 {
                        break;
                    }
                }
                last = end;
            }
        }

        if lines.is_empty() {
            return "\nFONCTION ProcessPaper(input):\n    état = InitialiserÉtat()\n    données = Prétraiter(input)\n    résultat = Calculer(état, données)\n    RETOURNER résultat\n".into();
        }

        format!("\nFONCTION Algorithme():\n{}\n", lines.join("\n"))
    }

    fn extract_complexity(text: &str) -> String {
        let lower = text.to_lowercase();

        // Search for Big-O notation
        let patterns = [
            r"O\(n\^?\d*\)",
            r"O\(n\s*log\s*n\)",
            r"O\(n\)",
            r"O\(1\)",
            r"O\(log\s*n\)",
            r"O\(2\^n\)",
            r"O\(n!\)",
            r"\bcomplexit[ée]\s+(?:en\s+)?(?:temporelle|spatiale|mémoire)?\s*[:\-]?\s*[OΘΩ][\(\{][^\)\}]+[\)\}]",
            r"\bruntime\b.{0,30}?[OΘΩ][\(\{][^\)\}]+[\)\}]",
            r"\btime\s+complexity\b.{0,30}?:?.{0,30}?[OΘΩ]",
        ];

        for pat in &patterns {
            if let Ok(re) = regex::Regex::new(pat) {
                if let Some(cap) = re.find(&lower) {
                    return cap.as_str().to_string();
                }
            }
        }

        "INFORMATION NON DISPONIBLE DANS LE PAPIER".into()
    }

    fn extract_memory_cost(text: &str) -> String {
        let lower = text.to_lowercase();

        let patterns = [
            r"O\(n\)\s*(?:memory|space|espace|mémoire)",
            r"O\(n\^?\d*\)\s*(?:memory|space|espace|mémoire)",
            r"(?:memory|space|espace|mémoire)\s+(?:complexity|cost|usage|footprint)\b.{0,50}?[OΘΩ]",
            r"\bstorage\b.{0,20}?requirement",
            r"\bmemory\s+cost\b.{0,30}",
            r"\b(?:MB|GB)\s*(?:of\s+)?(?:RAM|memory|VRAM)",
        ];

        for pat in &patterns {
            if let Ok(re) = regex::Regex::new(pat) {
                if let Some(cap) = re.find(&lower) {
                    return cap.as_str().to_string();
                }
            }
        }

        "INFORMATION NON DISPONIBLE DANS LE PAPIER".into()
    }
}

// ---------------------------------------------------------------------------
// Analyse système (heuristique basée sur #params et architecture)
// ---------------------------------------------------------------------------

pub struct SystemAnalyzer;

impl SystemAnalyzer {
    pub fn analyze(text: &str) -> SystemRequirements {
        let text_lower = text.to_lowercase();

        // Estimation du nombre de paramètres
        let param_count = Self::estimate_param_count(&text_lower);

        // VRAM estimation (basée sur #params, précision, batch size)
        let vram = Self::estimate_vram(param_count, &text_lower);

        // RAM estimation (pour inference, typiquement 2-4x le modèle)
        let ram = Self::estimate_ram(param_count, &text_lower);

        // Disk (taille du modèle sur disque)
        let disk = Self::estimate_disk(param_count, &text_lower);

        // Latence (estimation selon architecture)
        let latency = Self::estimate_latency(&text_lower);

        // Throughput
        let throughput = Self::estimate_throughput(&text_lower);

        // Scalabilité
        let scalability = Self::estimate_scalability(&text_lower);

        SystemRequirements {
            vram,
            ram,
            disk,
            latency,
            throughput,
            scalability,
        }
    }

    fn estimate_param_count(text: &str) -> Option<u64> {
        // Cherche des patterns comme "7B", "13B", "175B", "1.5B", "110M"
        let patterns = [
            (r"(\d+\.?\d*)\s*b\b", 1_000_000_000u64),
            (r"(\d+\.?\d*)\s*bn\b", 1_000_000_000u64),
            (r"(\d+\.?\d*)\s*m\b", 1_000_000u64),
            (r"(\d+\.?\d*)\s*millions?\b", 1_000_000u64),
            (r"(\d+\.?\d*)\s*billions?\b", 1_000_000_000u64),
            (r"(\d+\.?\d*)\s*trillions?\b", 1_000_000_000_000u64),
        ];

        for (pat, multiplier) in &patterns {
            if let Ok(re) = regex::Regex::new(pat) {
                if let Some(caps) = re.captures(text) {
                    if let Some(m) = caps.get(1) {
                        if let Ok(val) = m.as_str().parse::<f64>() {
                            return Some((val * *multiplier as f64) as u64);
                        }
                    }
                }
            }
        }
        None
    }

    fn estimate_vram(param_count: Option<u64>, text: &str) -> Option<String> {
        let params = param_count?;

        // fp32: 4 bytes/param, fp16: 2 bytes/param, int8: 1 byte/param
        let precision = if text.contains("int4") || text.contains("4-bit") || text.contains("qlora")
        {
            0.5
        } else if text.contains("int8") || text.contains("8-bit") {
            1.0
        } else if text.contains("fp16") || text.contains("half") || text.contains("float16") {
            2.0
        } else {
            4.0 // fp32 default
        };

        let bytes = params as f64 * precision;
        let gb = bytes / (1024.0 * 1024.0 * 1024.0);

        // Add ~20% overhead for activations/kv-cache
        let total = gb * 1.2;

        Some(format!("{:.1} GB", total))
    }

    fn estimate_ram(param_count: Option<u64>, text: &str) -> Option<String> {
        let params = param_count?;

        // Inference typically needs 2-4x model size for activations
        let multiplier = if text.contains("training") || text.contains("fine-tun") {
            8.0
        } else {
            3.0
        };

        let bytes = params as f64 * 4.0 * multiplier; // fp32
        let gb = bytes / (1024.0 * 1024.0 * 1024.0);

        Some(format!("{:.1} GB", gb))
    }

    fn estimate_disk(param_count: Option<u64>, text: &str) -> Option<String> {
        let params = param_count?;

        let bytes_per_param = if text.contains("int4") || text.contains("4-bit") {
            0.5
        } else if text.contains("int8") || text.contains("8-bit") {
            1.0
        } else if text.contains("fp16") || text.contains("half") {
            2.0
        } else {
            4.0
        };

        let bytes = params as f64 * bytes_per_param;
        let gb = bytes / (1024.0 * 1024.0 * 1024.0);

        Some(format!("{:.1} GB", gb))
    }

    fn estimate_latency(text: &str) -> Option<String> {
        if text.contains("transformer") || text.contains("attention") {
            Some("10-100 ms (GPU), 100-1000 ms (CPU)".into())
        } else if text.contains("cnn") || text.contains("convolution") {
            Some("1-10 ms (GPU)".into())
        } else if text.contains("rnn") || text.contains("lstm") || text.contains("gru") {
            Some("50-500 ms (sequential)".into())
        } else {
            None
        }
    }

    fn estimate_throughput(text: &str) -> Option<String> {
        if text.contains("transformer") || text.contains("llm") {
            Some("50-500 tokens/s (GPU, batch=1)".into())
        } else if text.contains("cnn") {
            Some("100-1000 images/s (GPU)".into())
        } else {
            None
        }
    }

    fn estimate_scalability(text: &str) -> Option<String> {
        if text.contains("distributed") || text.contains("parallel") || text.contains("multi-gpu") {
            Some("Good with data/model parallelism".into())
        } else if text.contains("transformer") || text.contains("attention") {
            Some("Moderate, requires careful memory management".into())
        } else if text.contains("cnn") {
            Some("Good with batch parallelism".into())
        } else {
            None
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
    pub fn map(
        document: &ExtractedDocument,
        executive_summary: &str,
        contributions: &[String],
    ) -> ArchitecturalMapping {
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
            (
                "memory",
                vec![
                    "memory",
                    "mémoire",
                    "kv cache",
                    "long-term memory",
                    "retrieval",
                    "rag",
                    "store",
                    "recall",
                ],
            ),
            (
                "perception",
                vec![
                    "perception",
                    "embedding",
                    "representation",
                    "encoding",
                    "encoder",
                    "feature",
                ],
            ),
            (
                "planning",
                vec![
                    "planning",
                    "planification",
                    "plan",
                    "search",
                    "mcts",
                    "tree",
                    "subgoal",
                ],
            ),
            (
                "decision",
                vec![
                    "decision",
                    "policy",
                    "choix",
                    "reasoning",
                    "raisonnement",
                    "inference",
                ],
            ),
            (
                "action",
                vec![
                    "action",
                    "tool use",
                    "act",
                    "execution",
                    "environment",
                    "agent",
                ],
            ),
            (
                "learning",
                vec![
                    "learning",
                    "apprentissage",
                    "training",
                    "fine-tuning",
                    "distillation",
                    "update",
                ],
            ),
            (
                "reflection",
                vec![
                    "reflection",
                    "reflect",
                    "self-correct",
                    "self-evaluation",
                    "introspection",
                ],
            ),
            (
                "evaluation",
                vec![
                    "evaluation",
                    "eval",
                    "benchmark",
                    "metric",
                    "reward",
                    "score",
                ],
            ),
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
        if !mapping.decision.is_empty()
            || !mapping.planning.is_empty()
            || !mapping.evaluation.is_empty()
        {
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
            .map(|t| {
                t.contains("```") || t.contains("fn ") || t.contains("def ") || t.contains("class ")
            })
            .unwrap_or(false);
        let has_full_text = document
            .full_text
            .as_deref()
            .map(|t| t.len() > 10000)
            .unwrap_or(false);
        let has_github = document.github_url.is_some();

        let documentation = if has_full_text { 0.8 } else { 0.5 };
        let code_available = if has_github { 0.5 } else { 0.0 };
        let code_examples = if has_code { 0.3 } else { 0.0 };

        // Normalisation par le score maximal atteignable (tous signaux présents)
        // pour que la reproductibilité s'exprime sur [0, 1] : sans cela, le
        // plafond effectif serait 0.4*0.8 + 0.3*0.5 + 0.3*0.3 = 0.56.
        const MAX_ATTAINABLE: f64 = 0.4 * 0.8 + 0.3 * 0.5 + 0.3 * 0.3;
        ((0.4 * documentation + 0.3 * code_available + 0.3 * code_examples) / MAX_ATTAINABLE)
            .min(1.0)
    }

    pub fn compute_integration(
        document: &ExtractedDocument,
        reproducibility: f64,
        has_equations: bool,
        has_references: bool,
    ) -> f64 {
        let code_avail = if document.github_url.is_some() {
            0.3
        } else {
            0.0
        };
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
                mitigation: Some(
                    "Contacter les auteurs ou tenter une reproduction indépendante.".into(),
                ),
            });
        }

        if text.is_empty() {
            risks.push(Risk {
                level: "HIGH".into(),
                description: "Texte complet non disponible, l'analyse repose sur l'abstract."
                    .into(),
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
    pub fn analyze(
        document: &ExtractedDocument,
        executive_summary: &str,
        contributions: &[String],
    ) -> HeuristicAnalysisResult {
        let text = document.full_text.as_deref().unwrap_or("");
        let math = MathematicalAnalyzer::analyze(text);
        let algo = AlgorithmAnalyzer::analyze(text);
        let sys = SystemAnalyzer::analyze(text);
        let arch = HeuristicArchitecturalMapper::map(document, executive_summary, contributions);
        let reproducibility = ScoringEngine::compute_reproducibility(document);
        let integration = ScoringEngine::compute_integration(
            document,
            reproducibility,
            !math.equations.is_empty(),
            !document.references.is_empty(),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Risk;

    fn doc(full_text: Option<&str>, github: bool) -> ExtractedDocument {
        ExtractedDocument {
            id: "A-1".into(),
            title: "Paper d'analyse".into(),
            authors: vec![],
            publication_date: None,
            source: "test".into(),
            paper_url: None,
            github_url: if github {
                Some("https://github.com/x/y".into())
            } else {
                None
            },
            abstract_text: None,
            full_text: full_text.map(|s| s.to_string()),
            references: vec![],
            parsed: None,
        }
    }

    #[test]
    fn mathematical_analyzer_extracts_equations_and_variables() {
        // NB : la regex des variables est gourmande ([^\n]+) → une affectation
        // par ligne maximum ; on place donc les affectations sur des lignes distinctes.
        let text = "La fonction objectif est $L = \\sum_i (y_i - f(x_i))^2$\n\
                    avec alpha = 0.01.\n\
                    La transformation mapping appliquée aux entrées.";
        let math = MathematicalAnalyzer::analyze(text);
        assert!(
            math.equations.iter().any(|e| e.contains("L =")),
            "équations: {:?}",
            math.equations
        );
        assert!(math.variables.iter().any(|(n, _)| n == "alpha"));
        assert!(
            !math.transformations.is_empty(),
            "mot-clé 'transformation' détecté"
        );
    }

    #[test]
    fn mathematical_analyzer_on_empty_text_is_safe() {
        let math = MathematicalAnalyzer::analyze("");
        assert!(math.equations.is_empty());
        assert!(math.variables.is_empty());
        assert!(math.loss_functions.is_empty());
    }

    #[test]
    fn scoring_reproducibility_bounds_and_signals() {
        // Aucun signal : documentation partielle seulement.
        let bare = ScoringEngine::compute_reproducibility(&doc(None, false));
        assert!((0.0..=1.0).contains(&bare));

        // GitHub + code + texte long : score maximal attendu.
        let long_code = format!("{}{}", "fn example() {{ }} ".repeat(600), "```");
        let rich = ScoringEngine::compute_reproducibility(&doc(Some(&long_code), true));
        assert!(rich > bare, "riche ({rich}) doit dépasser nu ({bare})");
        assert!(
            (rich - 1.0).abs() < 1e-9,
            "tous signaux présents → ~1.0, obtenu {rich}"
        );
    }

    #[test]
    fn scoring_integration_is_clamped_to_one() {
        let d = doc(Some("texte"), true);
        // repro=1.0, github=0.3, equations=0.2, refs=0.1 → somme ≥ 1 → clampée à 1.
        let s = ScoringEngine::compute_integration(&d, 1.0, true, true);
        assert!(
            (s - 1.0).abs() < 1e-9,
            "score clampé à 1 attendu, obtenu {s}"
        );
        // Cas minimal sans aucun signal.
        let bare_doc = doc(None, false);
        let s2 = ScoringEngine::compute_integration(&bare_doc, 0.0, false, false);
        assert!((s2 - 0.05).abs() < 1e-9);
    }

    #[test]
    fn risk_analyzer_flags_missing_code_and_text() {
        let risks = RiskAnalyzer::analyze(&doc(None, false), "");
        let levels: Vec<&str> = risks.iter().map(|r: &Risk| r.level.as_str()).collect();
        assert!(levels.contains(&"MEDIUM"), "pas de code → risque moyen");
        assert!(levels.contains(&"HIGH"), "pas de texte → risque haut");

        // Tout est fourni : aucun risque heuristique.
        let clean = RiskAnalyzer::analyze(&doc(Some("texte complet"), true), "texte complet");
        assert!(clean.is_empty());
    }

    #[test]
    fn heuristic_analyzer_end_to_end_consistency() {
        let text = "Nous proposons $E = mc^2$. L'algorithme utilise une boucle for. \
                    La loss objective minimise l'erreur. Performance sur GPU requise.";
        let d = doc(Some(text), false);
        let result = HeuristicAnalyzer::analyze(&d, "résumé", &["contribution".to_string()]);
        assert!((0.0..=1.0).contains(&result.reproducibility));
        assert!((0.0..=1.0).contains(&result.integration));
        // Pas de code associé → au moins un risque signalé.
        assert!(!result.risks.is_empty());

        // enrich_report propage les champs dans le rapport.
        let mut report = crate::engine::AnalysisReport {
            document: d,
            contributions: Vec::new(),
            executive_summary: String::new(),
            equations: Vec::new(),
            variables: Vec::new(),
            algorithms: Vec::new(),
            system_requirements: crate::engine::SystemRequirements {
                vram: None,
                ram: None,
                disk: None,
                latency: None,
                throughput: None,
                scalability: None,
            },
            risks: Vec::new(),
            recommendation: crate::engine::Recommendation::Reject,
            recommendation_justification: String::new(),
            integration_score: 0.0,
            reproducibility_score: 0.0,
            impacted_modules: Vec::new(),
            timestamp: String::new(),
            architectural_mapping: serde_json::Value::Null,
            deep_analysis: serde_json::Value::Null,
            experiment_plan: serde_json::Value::Null,
            pseudo_code: serde_json::Value::Null,
            llm_warnings: Vec::new(),
        };
        result.enrich_report(&mut report);
        assert!((report.integration_score - result.integration).abs() < 1e-12);
        assert!(!report.risks.is_empty());
        assert_eq!(report.algorithms.len(), 1);

        // architecture_json expose tous les piliers.
        let arch = result.architecture_json();
        for pillar in [
            "perception",
            "memory",
            "planning",
            "decision",
            "action",
            "learning",
        ] {
            assert!(arch.get(pillar).is_some(), "pilier {pillar} manquant");
        }
    }
}
