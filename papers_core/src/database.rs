use std::collections::HashMap;

use crate::models::Node;
use crate::samplers::{create_sampler, Sampler};
use crate::vector_store::VectorStore;

pub struct Database {
    nodes: HashMap<usize, Node>,
    next_id: usize,
    sampler_name: String,
    sampler: Box<dyn Sampler>,
    vector_store: Option<VectorStore>,
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
            vector_store: None,
        }
    }

    /// Crée une Database avec VectorStore activé pour la recherche sémantique.
    ///
    /// `corpus_texts` doit contenir les textes représentatifs du domaine
    /// (titres de papiers, abstracts) pour initialiser le vocabulaire du MiniLLM.
    pub fn new_with_vector_store(sampler_name: &str, corpus_texts: &[&str]) -> Self {
        Self {
            nodes: HashMap::new(),
            next_id: 0,
            sampler_name: sampler_name.to_string(),
            sampler: create_sampler(sampler_name),
            vector_store: Some(VectorStore::new(corpus_texts)),
        }
    }

    /// Vrai si le VectorStore est activé.
    pub fn has_vector_store(&self) -> bool {
        self.vector_store.is_some()
    }

    pub fn add(&mut self, mut node: Node) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        node.id = Some(id);

        // Indexer automatiquement dans le VectorStore si activé
        if let Some(ref mut vs) = self.vector_store {
            vs.add(id, &node.context_text());
        }

        self.nodes.insert(id, node);
        id
    }

    /// Recherche sémantique : retourne les `top_k` IDs les plus similaires à `query`.
    ///
    /// Retourne un vecteur vide si le VectorStore n'est pas activé.
    pub fn search_similar(&mut self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        match self.vector_store.as_mut() {
            Some(vs) => vs.search(query, top_k),
            None => Vec::new(),
        }
    }

    /// Recherche sémantique avec seuil de similarité minimal.
    pub fn search_similar_threshold(
        &mut self,
        query: &str,
        top_k: usize,
        threshold: f32,
    ) -> Vec<(usize, f32)> {
        match self.vector_store.as_mut() {
            Some(vs) => vs.search_with_threshold(query, top_k, threshold),
            None => Vec::new(),
        }
    }

    /// Dimension des embeddings (0 si pas de VectorStore).
    pub fn embedding_dim(&self) -> usize {
        self.vector_store.as_ref().map(|vs| vs.dim()).unwrap_or(0)
    }

    pub fn get(&self, id: usize) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn sample(&mut self, n: usize) -> Vec<Node> {
        let nodes: Vec<Node> = self.nodes.values().cloned().collect();
        let sampled = self.sampler.sample(&nodes, n);
        for node in &sampled {
            if let Some(id) = node.id {
                if let Some(stored) = self.nodes.get_mut(&id) {
                    stored.visit_count += 1;
                }
            }
        }
        sampled
    }

    pub fn best(&self) -> Option<&Node> {
        self.nodes
            .values()
            .max_by(|a, b| a.score.total_cmp(&b.score))
    }

    pub fn top_k(&self, k: usize) -> Vec<&Node> {
        let mut nodes: Vec<&Node> = self.nodes.values().collect();
        nodes.sort_by(|a, b| b.score.total_cmp(&a.score));
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
        entries.sort_by(|a, b| b.1.total_cmp(&a.1));
        let keep_ids: std::collections::HashSet<usize> = entries
            .into_iter()
            .take(keep_top)
            .map(|(id, _)| id)
            .collect();
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
        // VectorStore n'est pas clonable (EmbeddingEngine non-Clone)
        // Un clone perd la capacité de recherche sémantique.
        Self {
            nodes: self.nodes.clone(),
            next_id: self.next_id,
            sampler_name: self.sampler_name.clone(),
            sampler: create_sampler(&self.sampler_name),
            vector_store: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(motivation: &str, score: f64) -> Node {
        Node::new(
            motivation.to_string(),
            "fn candidate() { 1 + 1 }".to_string(),
            score,
        )
    }

    #[test]
    fn add_assigns_sequential_ids() {
        let mut db = Database::new("greedy");
        assert!(db.is_empty());
        let a = db.add(node("a", 0.1));
        let b = db.add(node("b", 0.2));
        assert_eq!((a, b), (0, 1));
        assert_eq!(db.get(a).unwrap().motivation, "a");
        assert_eq!(db.len(), 2);
    }

    #[test]
    fn best_and_top_k_ordering() {
        let mut db = Database::new("greedy");
        db.add(node("faible", 0.1));
        db.add(node("fort", 0.95));
        db.add(node("moyen", 0.5));

        assert_eq!(db.best().unwrap().motivation, "fort");
        let top = db.top_k(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].motivation, "fort");
        assert_eq!(top[1].motivation, "moyen");
    }

    #[test]
    fn stats_are_consistent() {
        let mut db = Database::new("greedy");
        for s in [0.0, 0.5, 1.0] {
            db.add(node("n", s));
        }
        let st = db.stats();
        assert_eq!(st.total_nodes, 3);
        assert!((st.mean_score - 0.5).abs() < 1e-9);
        assert_eq!(st.max_score, 1.0);
        assert_eq!(st.min_score, 0.0);
        assert!(st.std_score > 0.0);

        // Base vide : stats par défaut sans NaN.
        let empty = Database::new("greedy").stats();
        assert_eq!(empty.total_nodes, 0);
        assert!(empty.mean_score.is_finite());
    }

    #[test]
    fn sample_increments_visit_count_in_store() {
        let mut db = Database::new("greedy");
        let id = db.add(node("visité", 0.9));
        assert_eq!(db.get(id).unwrap().visit_count, 0);

        let sampled = db.sample(1);
        assert_eq!(sampled.len(), 1);
        assert_eq!(db.get(id).unwrap().visit_count, 1, "visite enregistrée");

        db.sample(1);
        assert_eq!(db.get(id).unwrap().visit_count, 2);
    }

    #[test]
    fn prune_bottom_keeps_only_top_nodes() {
        let mut db = Database::new("greedy");
        for i in 0..10 {
            db.add(node(&format!("n{i}"), i as f64 / 10.0));
        }
        db.prune_bottom(3);
        assert_eq!(db.len(), 3);
        let top: Vec<&str> = db.top_k(3).iter().map(|n| n.motivation.as_str()).collect();
        assert!(top.contains(&"n9") && top.contains(&"n8") && top.contains(&"n7"));

        // Prune sous le seuil : no-op.
        db.prune_bottom(100);
        assert_eq!(db.len(), 3);
    }

    #[test]
    fn to_json_roundtrip_preserves_nodes() {
        use crate::models::Node as N;
        let mut db = Database::new("ucb1");
        db.add(node("alpha", 0.42));
        db.add(node("beta", 0.77));

        let json = db.to_json();
        let parsed: Vec<N> = serde_json::from_str(&json).expect("JSON valide");
        assert_eq!(parsed.len(), 2);
        assert!(parsed
            .iter()
            .any(|p| p.motivation == "alpha" && p.score == 0.42));
    }

    #[test]
    fn semantic_search_without_vector_store_returns_empty() {
        let mut db = Database::new("greedy");
        db.add(node("deep learning", 0.5));
        assert!(!db.has_vector_store());
        assert_eq!(db.embedding_dim(), 0);
        assert!(db.search_similar("learning", 3).is_empty());
        assert!(db.search_similar_threshold("learning", 3, 0.1).is_empty());
    }

    #[test]
    fn clone_preserves_nodes_but_drops_vector_store() {
        let corpus = ["neural networks", "graph algorithms"];
        let mut db = Database::new_with_vector_store("greedy", &corpus);
        db.add(node("réseau de neurones convolutif", 0.6));
        assert!(db.has_vector_store());

        let cloned = db.clone();
        assert!(
            !cloned.has_vector_store(),
            "clone perd le VectorStore (documenté)"
        );
        assert_eq!(cloned.len(), 1);
        assert_eq!(cloned.next_id, db.next_id);
    }
}
