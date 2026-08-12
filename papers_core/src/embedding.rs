use std::collections::{HashMap, HashSet};

/// Deterministic, dependency-light embedding used by PAPERS core.
///
/// SciRust is deliberately no longer a build-time dependency of this module.
/// Higher-quality SciRust/CCOS retrieval backends should be connected through a
/// runtime adapter and benchmarked separately. This baseline remains replayable
/// and is suitable for tests, indexing and fallback retrieval.
pub struct PaperEmbeddingEngine {
    dim: usize,
    idf: HashMap<String, f32>,
}

impl PaperEmbeddingEngine {
    pub const DEFAULT_DIM: usize = 128;

    /// Build corpus-derived IDF weights with deterministic signed feature hashing.
    pub fn new(corpus_texts: &[&str]) -> Self {
        Self::with_dimension(corpus_texts, Self::DEFAULT_DIM)
    }

    /// Backward-compatible configuration entry point. The old argument was a
    /// SciRust MiniLLMConfig; PAPERS no longer interprets backend-specific config.
    pub fn with_config<T>(corpus_texts: &[&str], _config: T) -> Self {
        Self::new(corpus_texts)
    }

    pub fn with_dimension(corpus_texts: &[&str], dim: usize) -> Self {
        let dim = dim.max(8);
        let mut document_frequency: HashMap<String, usize> = HashMap::new();
        for text in corpus_texts {
            let unique: HashSet<String> = tokenize(text).into_iter().collect();
            for token in unique {
                *document_frequency.entry(token).or_default() += 1;
            }
        }
        let n = corpus_texts.len().max(1) as f32;
        let idf = document_frequency
            .into_iter()
            .map(|(token, df)| {
                let weight = ((n + 1.0) / (df as f32 + 1.0)).ln() + 1.0;
                (token, weight)
            })
            .collect();
        Self { dim, idf }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn embed(&mut self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0_f32; self.dim];
        let tokens = tokenize(text);
        if tokens.is_empty() {
            return vector;
        }

        let mut tf: HashMap<String, usize> = HashMap::new();
        for token in tokens {
            *tf.entry(token).or_default() += 1;
        }

        for (token, count) in tf {
            let hash = fnv1a64(token.as_bytes());
            let index = (hash as usize) % self.dim;
            let sign = if hash & (1 << 63) == 0 { 1.0 } else { -1.0 };
            let idf = self.idf.get(&token).copied().unwrap_or(1.0);
            let tf_weight = 1.0 + (count as f32).ln();
            vector[index] += sign * tf_weight * idf;
        }

        normalize_l2(&mut vector);
        vector
    }

    pub fn embed_batch(&mut self, texts: &[String]) -> Vec<Vec<f32>> {
        texts.iter().map(|text| self.embed(text)).collect()
    }

    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let an: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let bn: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if an <= f32::EPSILON || bn <= f32::EPSILON {
            0.0
        } else {
            (dot / (an * bn)).clamp(-1.0, 1.0)
        }
    }

    pub fn search(
        &mut self,
        query: &str,
        candidates: &[String],
        top_k: usize,
    ) -> Vec<(usize, f32)> {
        let q = self.embed(query);
        let mut scored: Vec<(usize, f32)> = candidates
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let v = self.embed(text);
                (index, Self::similarity(&q, &v))
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(top_k.min(scored.len()));
        scored
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize_l2(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_dimension_and_replay_are_stable() {
        let mut engine = PaperEmbeddingEngine::new(&["test paper about AI"]);
        let a = engine.embed("hello world");
        let b = engine.embed("hello world");
        assert_eq!(a.len(), 128);
        assert_eq!(a, b);
        assert!(a.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn embed_batch() {
        let mut engine = PaperEmbeddingEngine::new(&["test paper"]);
        let texts = vec!["a".into(), "b".into(), "c".into()];
        let results = engine.embed_batch(&texts);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|v| v.len() == 128));
    }

    #[test]
    fn similarity_identical_and_orthogonal() {
        assert!((PaperEmbeddingEngine::similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(PaperEmbeddingEngine::similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn search_prefers_shared_terms() {
        let mut engine = PaperEmbeddingEngine::new(&[
            "reinforcement learning",
            "graph neural networks",
            "symbolic regression",
        ]);
        let candidates = vec![
            "reinforcement learning agent".into(),
            "knowledge graph neural network".into(),
            "genetic programming".into(),
        ];
        let results = engine.search("reinforcement learning", &candidates, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn empty_string_is_zero_vector() {
        let mut engine = PaperEmbeddingEngine::new(&["test"]);
        assert_eq!(engine.embed(""), vec![0.0; 128]);
    }
}
