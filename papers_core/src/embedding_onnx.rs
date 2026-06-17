use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, Context};
use ort::session::Session;
use ort::value::TensorRef;

/// ONNX Runtime-based embedding engine using all-MiniLM-L6-v2 (384-dim).
///
/// Replaces the random-weight scirust `PaperEmbeddingEngine` with a real
/// sentence-transformer model running locally via ONNX Runtime.
pub struct OnnxEmbeddingEngine {
    session: Option<Mutex<Session>>,
    tokenizer: Tokenizer,
    dim: usize,
    model_path: std::path::PathBuf,
}

/// Simple tokenizer compatible with all-MiniLM-L6-v2.
#[derive(Clone)]
struct Tokenizer {
    vocab: HashMap<String, i64>,
    max_length: usize,
    cls_token_id: i64,
    pad_token_id: i64,
}

impl Tokenizer {
    fn new(vocab_path: Option<&Path>) -> Result<Self> {
        let (vocab, cls, pad) = if let Some(path) = vocab_path {
            Self::load_vocab(path)?
        } else {
            Self::default_vocab()
        };

        Ok(Self {
            vocab,
            max_length: 128,
            cls_token_id: cls,
            pad_token_id: pad,
        })
    }

    fn default_vocab() -> (HashMap<String, i64>, i64, i64) {
        let mut v = HashMap::new();
        v.insert("[CLS]".into(), 101);
        v.insert("[SEP]".into(), 102);
        v.insert("[PAD]".into(), 0);
        v.insert("[UNK]".into(), 103);
        (v, 101, 0)
    }

    fn load_vocab(path: &Path) -> Result<(HashMap<String, i64>, i64, i64)> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read vocab file")?;
        let mut vocab = HashMap::new();
        let mut cls = 101;
        let mut pad = 0;

        for (i, line) in content.lines().enumerate() {
            let token = line.trim().to_string();
            if token == "[CLS]" { cls = i as i64; }
            if token == "[PAD]" { pad = i as i64; }
            vocab.insert(token, i as i64);
        }

        Ok((vocab, cls, pad))
    }

    fn tokenize(&self, text: &str) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let mut input_ids = vec![self.cls_token_id];
        let mut attention_mask = vec![1i64];

        for word in words {
            if input_ids.len() >= self.max_length - 1 { break; }
            if let Some(id) = self.vocab.get(word) {
                input_ids.push(*id);
                attention_mask.push(1);
            } else {
                for ch in word.chars() {
                    if input_ids.len() >= self.max_length - 1 { break; }
                    let s = ch.to_string();
                    let id = self.vocab.get(s.as_str())
                        .copied().unwrap_or(self.vocab.get("[UNK]").copied().unwrap_or(103));
                    input_ids.push(id);
                    attention_mask.push(1);
                }
            }
        }

        let token_type_ids = vec![0i64; input_ids.len()];
        let mut ids = input_ids;
        let mut mask = attention_mask;
        let mut type_ids = token_type_ids;
        ids.resize(self.max_length, self.pad_token_id);
        mask.resize(self.max_length, 0);
        type_ids.resize(self.max_length, 0);
        (ids, mask, type_ids)
    }
}

impl OnnxEmbeddingEngine {
    /// Create a new embedding engine from an ONNX model path.
    pub fn new(model_dir: &Path) -> Result<Self> {
        let model_path = if model_dir.is_dir() {
            model_dir.join("model.onnx")
        } else {
            model_dir.to_path_buf()
        };

        let session = if model_path.exists() {
            match Session::builder()
                .context("Failed to create ONNX session builder")?
                .commit_from_file(&model_path)
            {
                Ok(s) => {
                    log::info!("Loaded ONNX model from {:?}", model_path);
                    Some(Mutex::new(s))
                }
                Err(e) => {
                    log::warn!("Failed to load ONNX model (will use fallback): {}", e);
                    None
                }
            }
        } else {
            log::warn!("ONNX model not found at {:?}, using deterministic fallback", model_path);
            None
        };

        let vocab_path = {
            let parent = model_path.parent().unwrap_or(Path::new("."));
            let v = parent.join("vocab.txt");
            if v.exists() { Some(v) } else { None }
        };
        let tokenizer = Tokenizer::new(vocab_path.as_deref())?;
        let dim = 384;

        Ok(Self { session, tokenizer, dim, model_path })
    }

    /// Try to load or reload the ONNX model if available.
    pub fn try_load_model(&mut self) -> Result<()> {
        if self.session.is_some() { return Ok(()); }
        if !self.model_path.exists() {
            anyhow::bail!("ONNX model not found at {:?}", self.model_path);
        }
        let session = Session::builder()?
            .commit_from_file(&self.model_path)?;
        self.session = Some(Mutex::new(session));
        Ok(())
    }

    /// Embed text using real ONNX inference, or deterministic fallback.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        match self.try_onnx_embed(text) {
            Ok(v) => v,
            Err(e) => {
                log::debug!("ONNX embed failed, using fallback: {}", e);
                self.fallback_embed(text)
            }
        }
    }

    /// Try ONNX inference.
    fn try_onnx_embed(&self, text: &str) -> Result<Vec<f32>> {
        let session = self.session.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No ONNX session loaded"))?;
        let (input_ids, attention_mask, token_type_ids) = self.tokenizer.tokenize(text);
        let seq_len = self.tokenizer.max_length;

        let ids_array = ndarray::Array2::from_shape_vec((1, seq_len), input_ids.clone())
            .context("Failed to create input_ids array")?;
        let mask_array = ndarray::Array2::from_shape_vec((1, seq_len), attention_mask.clone())
            .context("Failed to create attention_mask array")?;
        let type_ids_array = ndarray::Array2::from_shape_vec((1, seq_len), token_type_ids)
            .context("Failed to create token_type_ids array")?;

        let ids_ref = TensorRef::from_array_view(&ids_array)?;
        let mask_ref = TensorRef::from_array_view(&mask_array)?;
        let type_ids_ref = TensorRef::from_array_view(&type_ids_array)?;

        let mut sess = session.lock().unwrap();
        let outputs = sess.run(ort::inputs![ids_ref, mask_ref, type_ids_ref])?;

        let output_ref = outputs.iter().next()
            .ok_or_else(|| anyhow::anyhow!("No outputs from model"))?;
        let (shape, data) = output_ref.1.try_extract_tensor::<f32>()?;
        let num_dims = shape.len();

        let mut pooled = vec![0.0f32; self.dim];
        let mut mask_sum = 0.0f32;

        if num_dims == 3 {
            let seq_len_actual = shape[1] as usize;
            let embed_dim = self.dim.min(shape[2] as usize);
            for i in 0..seq_len_actual {
                let m = attention_mask.get(i).copied().unwrap_or(0) as f32;
                if m > 0.0 {
                    for j in 0..embed_dim {
                        pooled[j] += data[i * shape[2] as usize + j] * m;
                    }
                    mask_sum += m;
                }
            }
        } else if num_dims == 2 {
            let embed_dim = self.dim.min(shape[1] as usize);
            pooled[..embed_dim].copy_from_slice(&data[..embed_dim]);
            mask_sum = 1.0;
        }

        if mask_sum > 0.0 {
            for v in pooled.iter_mut() { *v /= mask_sum; }
        }

        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in pooled.iter_mut() { *v /= norm; }
        }

        Ok(pooled)
    }

    /// Deterministic hash-based fallback embedding.
    fn fallback_embed(&self, text: &str) -> Vec<f32> {
        let (input_ids, attention_mask, _) = self.tokenizer.tokenize(text);
        let mut embedding = vec![0.0f32; self.dim];
        let mut mask_sum = 0.0f32;
        for (i, &id) in input_ids.iter().enumerate() {
            let m = attention_mask.get(i).copied().unwrap_or(0) as f32;
            if m > 0.0 {
                mask_sum += m;
                let seed = (id as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                for (j, val) in embedding.iter_mut().enumerate().take(self.dim) {
                    let hash = seed.wrapping_add(j as u64 * 31);
                    let v = ((hash >> 33) as f32) / (i64::MAX as f32);
                    *val += v * m;
                }
            }
        }
        if mask_sum > 0.0 {
            for v in embedding.iter_mut() { *v /= mask_sum; }
        }
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in embedding.iter_mut() { *v /= norm; }
        }
        embedding
    }

    /// Embed a batch of texts.
    pub fn embed_batch(&self, texts: &[String]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Dimension of output vectors.
    pub fn dim(&self) -> usize { self.dim }

    /// Cosine similarity between two vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        if len == 0 { return 0.0; }
        let dot: f32 = a[..len].iter().zip(&b[..len]).map(|(x, y)| x * y).sum();
        let na: f32 = a[..len].iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b[..len].iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((OnnxEmbeddingEngine::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_tokenizer_basic() {
        let tok = Tokenizer::new(None).unwrap();
        let (ids, mask, type_ids) = tok.tokenize("hello world");
        assert_eq!(ids[0], 101);
        assert!(ids.len() <= 128);
        assert_eq!(ids.len(), mask.len());
        assert_eq!(ids.len(), type_ids.len());
    }

    #[test]
    fn test_embed_deterministic_fallback() {
        let engine = OnnxEmbeddingEngine {
            session: None,
            tokenizer: Tokenizer::new(None).unwrap(),
            dim: 384,
            model_path: PathBuf::from("dummy"),
        };
        let v1 = engine.embed("hello world");
        let v2 = engine.embed("hello world");
        assert_eq!(v1.len(), 384);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_embed_normalization() {
        let engine = OnnxEmbeddingEngine {
            session: None,
            tokenizer: Tokenizer::new(None).unwrap(),
            dim: 384,
            model_path: PathBuf::from("dummy"),
        };
        let v = engine.embed("test text");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "norm should be ~1.0, got {}", norm);
    }

    use std::path::PathBuf;
}
