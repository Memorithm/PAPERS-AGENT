use scirust_core::embed::EmbeddingEngine;
use scirust_core::nn::transformer::mini_llm::MiniLLMConfig;

/// Wrapper autour de `scirust_core::embed::EmbeddingEngine` pour le pipeline PAPERS.
///
/// Utilise les états cachés du MiniLLM (transformer char-level) comme vecteurs d'embedding.
/// Remplace `sentence-transformers/all-MiniLM-L6-v2` (384-dim) par un moteur local 128-dim.
pub struct PaperEmbeddingEngine {
    engine: EmbeddingEngine,
}

impl PaperEmbeddingEngine {
    /// Crée un moteur d'embedding initialisé avec le vocabulaire extrait des textes fournis.
    ///
    /// Les textes doivent inclure titres, abstracts, et contenus représentatifs
    /// du domaine pour construire un vocabulaire de caractères pertinent.
    pub fn new(corpus_texts: &[&str]) -> Self {
        let engine = EmbeddingEngine::new(corpus_texts);
        Self { engine }
    }

    /// Crée un moteur avec une config MiniLLM personnalisée.
    pub fn with_config(corpus_texts: &[&str], config: MiniLLMConfig) -> Self {
        let engine = EmbeddingEngine::new_with_config(corpus_texts, config);
        Self { engine }
    }

    /// Dimension des vecteurs d'embedding (défaut: 128).
    pub fn dim(&self) -> usize {
        self.engine.dim()
    }

    /// Embed un texte en vecteur f32 normalisé L2.
    pub fn embed(&mut self, text: &str) -> Vec<f32> {
        self.engine.embed(text)
    }

    /// Embed un batch de textes.
    pub fn embed_batch(&mut self, texts: &[String]) -> Vec<Vec<f32>> {
        self.engine.embed_batch(texts)
    }

    /// Cosine similarity entre deux vecteurs.
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        EmbeddingEngine::cosine_similarity(a, b)
    }

    /// Recherche les `top_k` textes les plus similaires à `query` parmi `candidates`.
    ///
    /// Retourne les indices des candidats et leurs scores de similarité cosinus.
    pub fn search(
        &mut self,
        query: &str,
        candidates: &[String],
        top_k: usize,
    ) -> Vec<(usize, f32)> {
        let q_vec = self.embed(query);
        let c_vecs = self.embed_batch(candidates);
        let mut scored: Vec<(usize, f32)> = c_vecs
            .iter()
            .enumerate()
            .map(|(i, v)| (i, Self::similarity(&q_vec, v)))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.into_iter().take(top_k).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dimension() {
        let mut engine = PaperEmbeddingEngine::new(&["test paper about AI"]);
        let vec = engine.embed("hello");
        assert_eq!(vec.len(), 128);
        assert!(vec.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_embed_batch() {
        let mut engine = PaperEmbeddingEngine::new(&["test paper"]);
        let texts: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let results = engine.embed_batch(&texts);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|v| v.len() == 128));
    }

    #[test]
    fn test_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((PaperEmbeddingEngine::similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(PaperEmbeddingEngine::similarity(&a, &b).abs() < 0.01);
    }

    #[test]
    fn test_search_basic() {
        let mut engine = PaperEmbeddingEngine::new(&[
            "reinforcement learning",
            "graph neural networks",
            "symbolic regression",
        ]);
        let candidates: Vec<String> = vec![
            "deep RL algorithms".into(),
            "GNN for knowledge graphs".into(),
            "genetic programming".into(),
        ];
        let results = engine.search("reinforcement learning", &candidates, 2);
        assert_eq!(results.len(), 2);
        // Les scores doivent être dans [-1, 1] et tous les indices doivent être valides
        for (idx, sim) in &results {
            assert!(*idx < candidates.len(), "index {} out of bounds", idx);
            assert!((-1.0..=1.0).contains(sim), "similarity {} out of range", sim);
        }
    }

    #[test]
    fn test_empty_string_handling() {
        let mut engine = PaperEmbeddingEngine::new(&["test"]);
        let vec = engine.embed("");
        assert_eq!(vec.len(), 128);
        assert!(vec.iter().all(|v| v.is_finite()));
    }
}
