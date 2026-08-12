use std::collections::{HashMap, HashSet};

use hnsw_rs::hnsw::Hnsw;
use hnsw_rs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::embedding::PaperEmbeddingEngine;

/// Entrée du vector store : un vecteur d'embedding associé à un ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorEntry {
    id: usize,
    #[serde(skip)]
    vector: Vec<f32>,
    /// Texte source utilisé pour générer l'embedding (pour référence).
    source_text: String,
}

/// Store de vecteurs avec recherche par similarité cosinus via HNSW.
///
/// Utilise un index HNSW (Hierarchical Navigable Small World) pour la
/// recherche ANN (Approximate Nearest Neighbors) au lieu d'une recherche
/// linéaire O(n). La complexité de recherche est ~O(log n).
pub struct VectorStore {
    entries: HashMap<usize, VectorEntry>,
    engine: PaperEmbeddingEngine,
    hnsw: Hnsw<'static, f32, DistL2>,
    removed: HashSet<usize>,
    next_id: usize,
}

impl std::fmt::Debug for VectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStore")
            .field("entries", &self.entries.len())
            .field("dim", &self.engine.dim())
            .finish()
    }
}

impl VectorStore {
    /// Crée un VectorStore avec vocabulaire extrait du corpus fourni.
    pub fn new(corpus_texts: &[&str]) -> Self {
        let _dim = 128; // MiniLLM default dimension
        let max_nb_connection = 16;
        let max_layer = 5;
        let max_elements = 1000;
        let ef_construction = 200;
        let hnsw = Hnsw::new(
            max_nb_connection,
            max_elements,
            max_layer,
            ef_construction,
            DistL2,
        );

        Self {
            entries: HashMap::new(),
            engine: PaperEmbeddingEngine::new(corpus_texts),
            hnsw,
            removed: HashSet::new(),
            next_id: 0,
        }
    }

    /// Dimension des vecteurs d'embedding.
    pub fn dim(&self) -> usize {
        self.engine.dim()
    }

    /// Ajoute ou met à jour un vecteur pour un ID donné.
    ///
    /// Le texte `source` est tokenizé et passé dans le MiniLLM pour
    /// produire un embedding 128-dim normalisé L2.
    pub fn add(&mut self, id: usize, source_text: &str) {
        let vector = self.engine.embed(source_text);

        // Insert into HNSW index
        self.hnsw.insert((&vector, id));

        self.entries.insert(
            id,
            VectorEntry {
                id,
                vector,
                source_text: source_text.to_string(),
            },
        );
        self.next_id = self.next_id.max(id + 1);
    }

    /// Ajoute un embedding pré-calculé (utile pour la sérialisation).
    pub fn add_precomputed(&mut self, id: usize, source_text: &str, vector: Vec<f32>) {
        self.hnsw.insert((&vector, id));

        self.entries.insert(
            id,
            VectorEntry {
                id,
                vector,
                source_text: source_text.to_string(),
            },
        );
        self.next_id = self.next_id.max(id + 1);
    }

    /// Recherche les `top_k` entrées les plus similaires à `query`.
    ///
    /// Utilise l'index HNSW pour une recherche ANN ~O(log n).
    pub fn search(&mut self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let q_vec = self.engine.embed(query);

        // Request extra results to compensate for filtered-out removed entries
        let ef_search = (top_k * 4).max(10);
        let neighbours =
            self.hnsw
                .search(&q_vec, top_k.saturating_add(self.removed.len()), ef_search);

        neighbours
            .iter()
            .filter(|n| !self.removed.contains(&n.d_id))
            .map(|n| {
                let l2_dist = n.distance;
                let similarity = 1.0 / (1.0 + l2_dist);
                (n.d_id, similarity)
            })
            .take(top_k)
            .collect()
    }

    /// Recherche avec un seuil minimal de similarité.
    pub fn search_with_threshold(
        &mut self,
        query: &str,
        top_k: usize,
        threshold: f32,
    ) -> Vec<(usize, f32)> {
        self.search(query, self.entries.len())
            .into_iter()
            .filter(|(_, sim)| *sim >= threshold)
            .take(top_k)
            .collect()
    }

    /// Supprime une entrée par ID.
    ///
    /// Note: HNSW ne supporte pas la suppression d'index. Les entrées supprimées
    /// sont masquées dans les résultats de recherche via un filtre.
    pub fn remove(&mut self, id: usize) {
        self.entries.remove(&id);
        self.removed.insert(id);
    }

    /// Nombre d'entrées dans le store.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Vrai si le store est vide.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Récupère le texte source d'une entrée.
    pub fn get_source(&self, id: usize) -> Option<&str> {
        self.entries.get(&id).map(|e| e.source_text.as_str())
    }
}

/// Représentation sérialisable du VectorStore (sans les vecteurs, trop volumineux).
#[derive(Debug, Serialize, Deserialize)]
pub struct VectorStoreSnapshot {
    pub dim: usize,
    pub entry_count: usize,
    pub entries: Vec<VectorEntrySnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VectorEntrySnapshot {
    pub id: usize,
    pub source_text: String,
    pub vector: Vec<f32>,
}

impl VectorStore {
    /// Exporte l'état complet (vecteurs inclus) pour sauvegarde.
    pub fn to_snapshot(&self) -> VectorStoreSnapshot {
        VectorStoreSnapshot {
            dim: self.engine.dim(),
            entry_count: self.entries.len(),
            entries: self
                .entries
                .values()
                .map(|e| VectorEntrySnapshot {
                    id: e.id,
                    source_text: e.source_text.clone(),
                    vector: e.vector.clone(),
                })
                .collect(),
        }
    }

    /// Reconstruit un VectorStore depuis un snapshot.
    pub fn from_snapshot(snapshot: VectorStoreSnapshot, corpus_texts: &[&str]) -> Self {
        let mut store = Self::new(corpus_texts);
        for entry in snapshot.entries {
            store.add_precomputed(entry.id, &entry.source_text, entry.vector);
        }
        store
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> VectorStore {
        VectorStore::new(&[
            "reinforcement learning",
            "graph neural networks",
            "symbolic regression",
            "natural language processing",
            "computer vision",
        ])
    }

    #[test]
    fn test_add_and_search() {
        let mut store = make_store();
        store.add(0, "reinforcement learning with deep Q-networks");
        store.add(1, "graph convolutional networks for node classification");
        store.add(2, "genetic programming for symbolic regression");

        let results = store.search("deep RL and Q-learning", 2);
        assert_eq!(results.len(), 2);
        for (idx, sim) in &results {
            assert!(*idx <= 2, "index {} out of bounds", idx);
            assert!(
                (-1.0..=1.0).contains(sim),
                "similarity {} out of range",
                sim
            );
        }
    }

    #[test]
    fn test_search_dissimilar() {
        let mut store = make_store();
        store.add(0, "deep reinforcement learning");
        store.add(1, "image classification with CNNs");
        store.add(2, "symbolic math regression");

        let results = store.search("computer vision and image recognition", 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].0 <= 2);
    }

    #[test]
    fn test_search_with_threshold() {
        let mut store = make_store();
        store.add(0, "reinforcement learning");
        store.add(1, "graph neural networks");
        store.add(2, "symbolic regression");

        let results = store.search_with_threshold("RL agents", 5, 0.0);
        assert!(!results.is_empty(), "should find at least one match");
    }

    #[test]
    fn test_remove() {
        let mut store = make_store();
        store.add(0, "test entry");
        assert_eq!(store.len(), 1);
        store.remove(0);
        assert!(store.is_empty());
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let mut store = make_store();
        store.add(0, "reinforcement learning");
        store.add(1, "graph neural networks");

        let snapshot = store.to_snapshot();
        let restored = VectorStore::from_snapshot(
            snapshot,
            &["reinforcement learning", "graph neural networks"],
        );

        assert_eq!(restored.len(), 2);
        assert_eq!(restored.get_source(0), Some("reinforcement learning"));
    }

    #[test]
    fn test_get_source() {
        let mut store = make_store();
        store.add(42, "transformers and attention");
        assert_eq!(store.get_source(42), Some("transformers and attention"));
        assert_eq!(store.get_source(99), None);
    }
}
