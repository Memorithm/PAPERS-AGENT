use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Option<usize>,
    pub name: String,
    pub created_at: String,
    pub parent: Vec<usize>,
    pub motivation: String,
    pub code: String,
    pub results: HashMap<String, serde_json::Value>,
    pub analysis: String,
    pub score: f64,
    pub visit_count: usize,
}

impl Node {
    pub fn new(motivation: String, code: String, score: f64) -> Self {
        Self {
            id: None,
            name: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            parent: Vec::new(),
            motivation,
            code,
            results: HashMap::new(),
            analysis: String::new(),
            score,
            visit_count: 0,
        }
    }

    pub fn context_text(&self) -> String {
        format!("{} {} {}", self.name, self.motivation, self.analysis)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitionItem {
    pub id: Option<String>,
    pub content: String,
    pub source: String,
    pub tags: Vec<String>,
}

impl CognitionItem {
    pub fn new(content: String, source: String, tags: Vec<String>) -> Self {
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            content,
            source,
            tags,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    pub task_description: String,
    pub max_rounds: usize,
    pub sampling_policy: String,
    pub n_candidates_per_round: usize,
    pub n_context_nodes: usize,
    pub n_cognition: usize,
    pub patience: usize,
    pub target_score: Option<f64>,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            task_description: String::new(),
            max_rounds: 50,
            sampling_policy: "greedy".into(),
            n_candidates_per_round: 3,
            n_context_nodes: 5,
            n_cognition: 5,
            patience: 10,
            target_score: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionResult {
    pub success: bool,
    pub best_score: f64,
    pub best_node: Option<Node>,
    pub total_rounds: usize,
    pub total_candidates: usize,
    pub total_time_secs: f64,
    pub stopped_early: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FalsificationCheck {
    pub hypothesis_id: String,
    pub score: f64,
    pub falsifiability: f64,
    pub risk: String,
    pub explanation: String,
}
