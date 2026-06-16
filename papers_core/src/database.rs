use std::collections::HashMap;

use crate::models::Node;
use crate::samplers::{create_sampler, Sampler};

pub struct Database {
    nodes: HashMap<usize, Node>,
    next_id: usize,
    sampler_name: String,
    sampler: Box<dyn Sampler>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("nodes", &self.nodes.len())
            .field("next_id", &self.next_id)
            .field("sampler_name", &self.sampler_name)
            .finish()
    }
}

impl Database {
    pub fn new(sampler_name: &str) -> Self {
        Self {
            nodes: HashMap::new(),
            next_id: 0,
            sampler_name: sampler_name.to_string(),
            sampler: create_sampler(sampler_name),
        }
    }

    pub fn add(&mut self, mut node: Node) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        node.id = Some(id);
        self.nodes.insert(id, node);
        id
    }

    pub fn get(&self, id: usize) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn sample(&self, n: usize) -> Vec<Node> {
        let nodes: Vec<Node> = self.nodes.values().cloned().collect();
        self.sampler.sample(&nodes, n)
    }

    pub fn best(&self) -> Option<&Node> {
        self.nodes
            .values()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn top_k(&self, k: usize) -> Vec<&Node> {
        let mut nodes: Vec<&Node> = self.nodes.values().collect();
        nodes.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        nodes.into_iter().take(k).collect()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn stats(&self) -> DatabaseStats {
        let scores: Vec<f64> = self.nodes.values().map(|n| n.score).collect();
        let n = scores.len() as f64;
        if n == 0.0 {
            return DatabaseStats::default();
        }
        let mean = scores.iter().sum::<f64>() / n;
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n;
        DatabaseStats {
            total_nodes: self.nodes.len(),
            mean_score: mean,
            max_score: scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            min_score: scores.iter().cloned().fold(f64::INFINITY, f64::min),
            std_score: variance.sqrt(),
        }
    }

    pub fn prune_bottom(&mut self, keep_top: usize) {
        if self.nodes.len() <= keep_top {
            return;
        }
        let mut entries: Vec<(usize, f64)> = self
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.score))
            .collect();
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let keep_ids: std::collections::HashSet<usize> =
            entries.into_iter().take(keep_top).map(|(id, _)| id).collect();
        self.nodes.retain(|id, _| keep_ids.contains(id));
    }

    pub fn to_json(&self) -> String {
        let nodes: Vec<&Node> = self.nodes.values().collect();
        serde_json::to_string_pretty(&nodes).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DatabaseStats {
    pub total_nodes: usize,
    pub mean_score: f64,
    pub max_score: f64,
    pub min_score: f64,
    pub std_score: f64,
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            next_id: self.next_id,
            sampler_name: self.sampler_name.clone(),
            sampler: create_sampler(&self.sampler_name),
        }
    }
}
