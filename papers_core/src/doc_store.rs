use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::embedding::PaperEmbeddingEngine;

/// Document stocké avec embedding, texte source et métadonnées.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDocument {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Store de documents persistant avec recherche sémantique.
///
/// Remplace `chromadb` + `sentence-transformers` par une solution 100% Rust.
/// Utilise `scirust-core::EmbeddingEngine` pour les embeddings.
pub struct DocStore {
    documents: Vec<StoredDocument>,
    engine: PaperEmbeddingEngine,
    persist_path: Option<PathBuf>,
}

impl std::fmt::Debug for DocStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocStore")
            .field("documents", &self.documents.len())
            .field("dim", &self.engine.dim())
            .field("persist_path", &self.persist_path)
            .finish()
    }
}

impl DocStore {
    /// Crée un DocStore avec vocabulaire initialisé à partir des textes fournis.
    pub fn new(corpus_texts: &[&str]) -> Self {
        Self {
            documents: Vec::new(),
            engine: PaperEmbeddingEngine::new(corpus_texts),
            persist_path: None,
        }
    }

    /// Crée un DocStore avec persistance automatique sur disque.
    ///
    /// Si le fichier existe, les documents sont chargés au démarrage.
    pub fn with_persistence(corpus_texts: &[&str], persist_path: &Path) -> std::io::Result<Self> {
        let mut store = Self {
            documents: Vec::new(),
            engine: PaperEmbeddingEngine::new(corpus_texts),
            persist_path: Some(persist_path.to_path_buf()),
        };

        if persist_path.exists() {
            let json = std::fs::read_to_string(persist_path)?;
            if !json.trim().is_empty() {
                store.documents = serde_json::from_str(&json)
                    .unwrap_or_default();
            }
            log::info!("DocStore chargé: {} documents depuis {}", store.documents.len(), persist_path.display());
        }

        Ok(store)
    }

    /// Dimension des embeddings.
    pub fn dim(&self) -> usize {
        self.engine.dim()
    }

    /// Nombre de documents stockés.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Vrai si le store est vide.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Ajoute un document avec métadonnées optionnelles.
    ///
    /// L'embedding est calculé automatiquement via le MiniLLM char-level.
    pub fn add(
        &mut self,
        id: &str,
        text: &str,
        metadata: HashMap<String, serde_json::Value>,
    ) {
        // Supprimer l'ancien document avec le même ID s'il existe
        self.remove(id);

        let embedding = self.engine.embed(text);
        self.documents.push(StoredDocument {
            id: id.to_string(),
            text: text.to_string(),
            embedding,
            metadata,
        });

        self.maybe_persist();
    }

    /// Ajoute un document avec un embedding pré-calculé.
    pub fn add_with_embedding(
        &mut self,
        id: &str,
        text: &str,
        embedding: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) {
        self.remove(id);
        self.documents.push(StoredDocument {
            id: id.to_string(),
            text: text.to_string(),
            embedding,
            metadata,
        });
        self.maybe_persist();
    }

    /// Supprime un document par ID.
    pub fn remove(&mut self, id: &str) {
        self.documents.retain(|d| d.id != id);
        self.maybe_persist();
    }

    /// Récupère un document par ID.
    pub fn get(&self, id: &str) -> Option<&StoredDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    /// Recherche sémantique : retourne les `top_k` documents les plus similaires.
    ///
    /// Retourne une liste de (ID, score de similarité cosinus, métadonnées).
    pub fn search(
        &mut self,
        query: &str,
        top_k: usize,
    ) -> Vec<SearchResult> {
        let q_vec = self.engine.embed(query);
        self.search_by_vector(&q_vec, top_k)
    }

    /// Recherche sémantique avec seuil minimal de similarité.
    pub fn search_with_threshold(
        &mut self,
        query: &str,
        top_k: usize,
        min_similarity: f32,
    ) -> Vec<SearchResult> {
        self.search(query, self.documents.len())
            .into_iter()
            .filter(|r| r.similarity >= min_similarity)
            .take(top_k)
            .collect()
    }

    /// Recherche par vecteur d'embedding pré-calculé.
    pub fn search_by_vector(
        &self,
        query_vector: &[f32],
        top_k: usize,
    ) -> Vec<SearchResult> {
        let mut scored: Vec<(usize, f32)> = self
            .documents
            .iter()
            .enumerate()
            .map(|(idx, doc)| {
                let sim = PaperEmbeddingEngine::similarity(query_vector, &doc.embedding);
                (idx, sim)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(idx, sim)| {
                let doc = &self.documents[idx];
                SearchResult {
                    id: doc.id.clone(),
                    similarity: sim,
                    text: doc.text.clone(),
                    metadata: doc.metadata.clone(),
                }
            })
            .collect()
    }

    /// Recherche avec filtrage par métadonnées.
    ///
    /// Seuls les documents dont les métadonnées correspondent au filtre sont considérés.
    /// Le filtre vérifie l'égalité pour chaque clé/valeur.
    pub fn search_with_filter(
        &mut self,
        query: &str,
        top_k: usize,
        filter: &HashMap<String, serde_json::Value>,
    ) -> Vec<SearchResult> {
        let q_vec = self.engine.embed(query);
        let mut scored: Vec<(usize, f32)> = self
            .documents
            .iter()
            .enumerate()
            .filter(|(_, doc)| {
                filter.iter().all(|(k, v)| {
                    doc.metadata.get(k).map(|mv| mv == v).unwrap_or(false)
                })
            })
            .map(|(idx, doc)| {
                let sim = PaperEmbeddingEngine::similarity(&q_vec, &doc.embedding);
                (idx, sim)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(idx, sim)| {
                let doc = &self.documents[idx];
                SearchResult {
                    id: doc.id.clone(),
                    similarity: sim,
                    text: doc.text.clone(),
                    metadata: doc.metadata.clone(),
                }
            })
            .collect()
    }

    /// Sauvegarde tous les documents sur le disque (si un chemin de persistance est configuré).
    pub fn persist(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.persist_path {
            let json = serde_json::to_string_pretty(&self.documents)
                .unwrap_or_default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, json)?;
            log::info!("DocStore persisté: {} documents -> {}", self.documents.len(), path.display());
        }
        Ok(())
    }

    fn maybe_persist(&self) {
        if self.persist_path.is_some() {
            let _ = self.persist();
        }
    }
}

/// Résultat d'une recherche sémantique.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub similarity: f32,
    pub text: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> DocStore {
        DocStore::new(&[
            "reinforcement learning neural networks",
            "graph neural networks knowledge graphs",
            "symbolic regression genetic programming",
        ])
    }

    #[test]
    fn test_add_and_search() {
        let mut store = make_store();

        let mut meta1 = HashMap::new();
        meta1.insert("domain".to_string(), serde_json::json!("RL"));
        store.add("paper-1", "deep reinforcement learning with Q-networks", meta1);

        let mut meta2 = HashMap::new();
        meta2.insert("domain".to_string(), serde_json::json!("GNN"));
        store.add("paper-2", "graph convolutional networks for node classification", meta2);

        let results = store.search("reinforcement learning", 2);
        assert_eq!(results.len(), 2);
        assert!((-1.0..=1.0).contains(&results[0].similarity));
    }

    #[test]
    fn test_search_with_filter() {
        let mut store = make_store();

        let mut meta_rl = HashMap::new();
        meta_rl.insert("domain".to_string(), serde_json::json!("RL"));
        store.add("p1", "Q-learning agents", meta_rl);

        let mut meta_gnn = HashMap::new();
        meta_gnn.insert("domain".to_string(), serde_json::json!("GNN"));
        store.add("p2", "graph attention networks", meta_gnn);

        let mut filter = HashMap::new();
        filter.insert("domain".to_string(), serde_json::json!("RL"));

        let results = store.search_with_filter("neural attention", 5, &filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "p1");
    }

    #[test]
    fn test_remove_and_get() {
        let mut store = make_store();
        store.add("test-id", "some text", HashMap::new());
        assert!(store.get("test-id").is_some());
        store.remove("test-id");
        assert!(store.get("test-id").is_none());
    }

    #[test]
    fn test_persistence_roundtrip() {
        let dir = std::env::temp_dir().join("papers_test_docstore");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("docs.json");

        let corpus = &["test corpus"];
        {
            let mut store = DocStore::with_persistence(corpus, &path).unwrap();
            store.add("doc-1", "reinforcement learning paper", HashMap::new());
            store.add("doc-2", "graph neural network paper", HashMap::new());
            store.persist().unwrap();
        }

        // Recharger
        let mut store = DocStore::with_persistence(corpus, &path).unwrap();
        assert_eq!(store.len(), 2);

        let results = store.search("RL algorithms", 1);
        assert_eq!(results.len(), 1);

        // Nettoyage
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_search_with_threshold() {
        let mut store = make_store();
        store.add("d1", "reinforcement learning", HashMap::new());
        store.add("d2", "graph neural networks", HashMap::new());

        // Avec un seuil bas (-1.0), tous les documents devraient être retournés
        let results = store.search_with_threshold("machine learning", 5, -1.0);
        assert_eq!(results.len(), 2);

        // Avec un seuil très élevé, aucun document ne passe
        let results_high = store.search_with_threshold("machine learning", 5, 2.0);
        assert_eq!(results_high.len(), 0);

        // Vérifier que chaque résultat a un score valide
        for r in &results {
            assert!((-1.0..=1.0).contains(&r.similarity));
        }
    }
}
