//! Cache persistant d'embeddings.
//!
//! Évite de recalculer les vecteurs (ONNX MiniLM notamment, coûteux) entre
//! les exécutions. Les entrées sont clé-valeur :
//!
//! - clé : SHA-256 de `"{dim}|{texte}"` — la dimension fait partie de la clé
//!   pour empêcher toute collision entre moteurs différents ;
//! - valeur : vecteur f32 sérialisé en JSON.
//!
//! Le fichier est tolérant aux corruptions : un contenu illisible repart à
//! vide avec un avertissement, sans jamais bloquer l'embedding.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::scientific_contract::sha256_hex;

/// Cache disque d'embeddings.
pub struct EmbeddingCache {
    path: PathBuf,
    dim: usize,
    entries: HashMap<String, Vec<f32>>,
    dirty: bool,
}

impl EmbeddingCache {
    /// Charge (ou initialise) le cache pour une dimension donnée.
    pub fn load<P: AsRef<Path>>(path: P, dim: usize) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut entries = HashMap::new();
        if path.exists() {
            match fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|content| {
                    serde_json::from_str::<HashMap<String, Vec<f32>>>(&content)
                        .map_err(|e| e.to_string())
                }) {
                Ok(loaded) => entries = loaded,
                Err(e) => {
                    log::warn!(
                        "EmbeddingCache illisible ({}): {} — démarrage à vide",
                        path.display(),
                        e
                    );
                }
            }
        }
        Self {
            path,
            dim,
            entries,
            dirty: false,
        }
    }

    fn key(&self, text: &str) -> String {
        sha256_hex(format!("{}|{}", self.dim, text).as_bytes())
    }

    /// Récupère le vecteur caché s'il existe.
    pub fn get(&self, text: &str) -> Option<Vec<f32>> {
        self.entries.get(&self.key(text)).cloned()
    }

    /// Insère (ou remplace) un vecteur. La sauvegarde est différée ([`save`]).
    pub fn put(&mut self, text: &str, vector: Vec<f32>) {
        self.entries.insert(self.key(text), vector);
        self.dirty = true;
    }

    /// Nombre d'entrées.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sauvegarde atomique (tmp + rename). No-op si rien n'a changé.
    pub fn save(&self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| format!("création dossier: {e}"))?;
            }
        }
        let json =
            serde_json::to_string(&self.entries).map_err(|e| format!("sérialisation: {e}"))?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, json).map_err(|e| format!("écriture temporaire: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename atomique: {e}"))?;
        Ok(())
    }
}

impl Drop for EmbeddingCache {
    fn drop(&mut self) {
        // Best effort : ne masque jamais une erreur applicative.
        let _ = self.save();
    }
}

/// Enveloppe un moteur d'embedding quelconque derrière le cache disque.
///
/// Compte hits/misses pour mesurer l'efficacité réelle du cache.
pub struct CachedEmbedder<'a> {
    embed_fn: &'a mut dyn FnMut(&str) -> Vec<f32>,
    dim: usize,
    cache: EmbeddingCache,
    hits: usize,
    misses: usize,
}

impl<'a> CachedEmbedder<'a> {
    /// Construit un cache autour d'une fonction d'embedding.
    pub fn new<F>(path: &Path, dim: usize, embed_fn: &'a mut F) -> Self
    where
        F: FnMut(&str) -> Vec<f32>,
    {
        Self {
            embed_fn,
            dim,
            cache: EmbeddingCache::load(path, dim),
            hits: 0,
            misses: 0,
        }
    }

    /// Embedding avec lecture/écriture transparente du cache.
    pub fn embed(&mut self, text: &str) -> Vec<f32> {
        if let Some(v) = self.cache.get(text) {
            self.hits += 1;
            return v;
        }
        self.misses += 1;
        let v = (self.embed_fn)(text);
        self.cache.put(text, v.clone());
        v
    }

    pub fn embed_batch(&mut self, texts: &[String]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    /// (hits, misses) depuis la création.
    pub fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }

    /// Force l'écriture immédiate du cache sur disque.
    pub fn flush(&self) -> Result<(), String> {
        self.cache.save()
    }

    /// Nombre d'entrées actuellement cachées.
    pub fn cached_entries(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn fake_embed(text: &str) -> Vec<f32> {
        // Vecteur pseudo-déterministe basé sur la longueur + premier caractère.
        let base = text.len() as f32;
        vec![base, 1.0, 0.5]
    }

    #[test]
    fn cache_roundtrip_across_instances() {
        let dir = std::env::temp_dir().join(format!("papers_embcache_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emb.json");

        let counter = Cell::new(0usize);
        let mut compute = |text: &str| {
            counter.set(counter.get() + 1);
            fake_embed(text)
        };
        {
            let mut emb = CachedEmbedder::new(&path, 3, &mut compute);
            assert_eq!(emb.embed("hello world"), fake_embed("hello world"));
            assert_eq!(
                emb.embed("hello world"),
                fake_embed("hello world"),
                "2e appel: hit attendu"
            );
            assert_eq!(emb.stats(), (1, 1));
            emb.flush().unwrap();
        }

        // Nouvelle instance : le texte doit venir du disque sans recalcul.
        let mut compute2 = |_t: &str| {
            counter.set(counter.get() + 1);
            fake_embed(_t)
        };
        {
            let mut emb = CachedEmbedder::new(&path, 3, &mut compute2);
            assert_eq!(emb.embed("hello world"), fake_embed("hello world"));
            assert_eq!(emb.stats(), (1, 0), "hit depuis disque sans recalcul");
        }
        assert_eq!(counter.get(), 1, "un seul calcul au total");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_cache_file_starts_empty_without_panic() {
        let dir =
            std::env::temp_dir().join(format!("papers_embcache_corrupt_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emb.json");
        fs::write(&path, "%%% pas du json %%%").unwrap();

        let cache = EmbeddingCache::load(&path, 128);
        assert!(cache.is_empty());
        // Un put + save doit reconstruire un fichier valide.
        let mut cache = EmbeddingCache::load(&path, 3);
        cache.put("x", vec![0.1; 3]);
        cache.save().unwrap();
        assert_eq!(EmbeddingCache::load(&path, 3).len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dimension_is_part_of_the_key() {
        let dir = std::env::temp_dir().join(format!("papers_embcache_dim_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emb.json");

        let mut c = |_t: &str| vec![0.0; _t.len().min(8)];
        {
            let mut a = CachedEmbedder::new(&path, 8, &mut c);
            a.embed("texte commun");
            a.flush().unwrap();
        }
        // Dimension différente → pas de faux hit.
        let mut miss_count = 0;
        let mut c2 = |t: &str| {
            miss_count += 1;
            vec![0.0; t.len().min(4)]
        };
        {
            let mut b = CachedEmbedder::new(&path, 4, &mut c2);
            b.embed("texte commun");
            assert_eq!(b.stats(), (0, 1));
            assert_eq!(b.dim(), 4);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn batch_and_drop_flush() {
        let dir =
            std::env::temp_dir().join(format!("papers_embcache_batch_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("emb.json");

        let mut c = fake_embed;
        {
            let mut emb = CachedEmbedder::new(&path, 3, &mut c);
            let out = emb.embed_batch(&["a".into(), "b".into()]);
            assert_eq!(out.len(), 2);
            assert_eq!(emb.cached_entries(), 2);
            // Pas de flush explicite : Drop doit persister.
        }
        assert_eq!(EmbeddingCache::load(&path, 3).len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
