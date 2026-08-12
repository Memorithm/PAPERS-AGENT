//! Registre de papiers avec persistance JSON et import vers l'écosystème PAPERS.
//!
//! Porte `knowledge/papers_registry.py` du Python.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::cognition::CognitionStore;
use crate::graph::GraphMiner;
use crate::models::CognitionItem;
use crate::vector_store::VectorStore;

/// Une entrée de registre pour un papier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperEntry {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub abstract_text: Option<String>,
    #[serde(default)]
    pub key_insight: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub relevance_score: Option<f64>,
    #[serde(default)]
    pub github_url: Option<String>,
}

/// Métriques du registre.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_papers: usize,
    pub by_year: HashMap<i32, usize>,
    pub by_domain: HashMap<String, usize>,
    pub total_tags: usize,
}

/// Registre persistant de papiers.
pub struct PaperRegistry {
    papers: Vec<PaperEntry>,
    path: std::path::PathBuf,
}

impl PaperRegistry {
    /// Charge ou crée un registre au chemin donné.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        let papers = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    warn!("Erreur parse PaperRegistry: {}", e);
                    Vec::new()
                }),
                Err(e) => {
                    warn!("Erreur lecture PaperRegistry: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        info!("PaperRegistry chargé: {} papiers", papers.len());
        Self { papers, path }
    }

    /// Sauvegarde le registre sur le disque.
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Création dossier: {}", e))?;
        }
        let json = serde_json::to_string_pretty(&self.papers).map_err(|e| format!("Sérialisation: {}", e))?;
        fs::write(&self.path, &json).map_err(|e| format!("Écriture: {}", e))?;
        info!("PaperRegistry sauvegardé: {} papiers", self.papers.len());
        Ok(())
    }

    /// Ajoute un papier (évite les doublons par ID).
    pub fn add(&mut self, paper: PaperEntry) {
        if !self.papers.iter().any(|p| p.id == paper.id) {
            self.papers.push(paper);
        }
    }

    /// Récupère un papier par ID.
    pub fn get(&self, id: &str) -> Option<&PaperEntry> {
        self.papers.iter().find(|p| p.id == id)
    }

    /// Recherche textuelle simple.
    pub fn search(&self, query: &str) -> Vec<&PaperEntry> {
        let q = query.to_lowercase();
        self.papers
            .iter()
            .filter(|p| {
                let haystack = format!(
                    "{} {} {} {}",
                    p.title.to_lowercase(),
                    p.tags.join(" ").to_lowercase(),
                    p.abstract_text.as_deref().unwrap_or("").to_lowercase(),
                    p.authors.join(" ").to_lowercase(),
                );
                haystack.contains(&q)
            })
            .collect()
    }

    /// Filtre par tag.
    pub fn filter_by_tag(&self, tag: &str) -> Vec<&PaperEntry> {
        self.papers
            .iter()
            .filter(|p| p.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Filtre par domaine.
    pub fn filter_by_domain(&self, domain: &str) -> Vec<&PaperEntry> {
        self.papers
            .iter()
            .filter(|p| p.domain.as_deref() == Some(domain))
            .collect()
    }

    /// Filtre par année.
    pub fn filter_by_year(&self, year: i32) -> Vec<&PaperEntry> {
        self.papers
            .iter()
            .filter(|p| p.year == Some(year))
            .collect()
    }

    /// Liste tous les tags uniques.
    pub fn list_tags(&self) -> Vec<String> {
        let mut tags: HashSet<&str> = HashSet::new();
        for p in &self.papers {
            for t in &p.tags {
                tags.insert(t);
            }
        }
        let mut sorted: Vec<String> = tags.into_iter().map(|s| s.to_string()).collect();
        sorted.sort();
        sorted
    }

    /// Liste tous les domaines uniques.
    pub fn list_domains(&self) -> Vec<String> {
        let mut domains: HashSet<&str> = HashSet::new();
        for p in &self.papers {
            if let Some(ref d) = p.domain {
                domains.insert(d);
            }
        }
        let mut sorted: Vec<String> = domains.into_iter().map(|s| s.to_string()).collect();
        sorted.sort();
        sorted
    }

    /// Statistiques du registre.
    pub fn stats(&self) -> RegistryStats {
        let mut by_year: HashMap<i32, usize> = HashMap::new();
        let mut by_domain: HashMap<String, usize> = HashMap::new();

        for p in &self.papers {
            if let Some(y) = p.year {
                *by_year.entry(y).or_insert(0) += 1;
            }
            let domain = p.domain.clone().unwrap_or_else(|| "unknown".into());
            *by_domain.entry(domain).or_insert(0) += 1;
        }

        RegistryStats {
            total_papers: self.papers.len(),
            by_year,
            by_domain,
            total_tags: self.list_tags().len(),
        }
    }

    /// Nombre de papiers.
    pub fn len(&self) -> usize {
        self.papers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.papers.is_empty()
    }

    // -----------------------------------------------------------------------
    // Import vers l'écosystème PAPERS
    // -----------------------------------------------------------------------

    /// Importe tous les papiers vers CognitionStore.
    pub fn import_to_cognition(&self, store: &mut CognitionStore) -> usize {
        let mut count = 0;
        for paper in &self.papers {
            let content = [
                format!("Title: {}", paper.title),
                format!("Authors: {}", paper.authors.join(", ")),
                format!("Key Insight: {}", paper.key_insight.as_deref().unwrap_or("")),
                paper.abstract_text.as_deref().unwrap_or("").to_string(),
            ]
            .join("\n\n");

            store.add(CognitionItem::new(
                content,
                paper
                    .source
                    .clone()
                    .unwrap_or_else(|| paper.title.clone()),
                paper.tags.clone(),
            ));
            count += 1;
        }
        info!("Importé {} papiers vers CognitionStore", count);
        count
    }

    /// Importe tous les papiers vers GraphMiner.
    pub fn import_to_graph(&self, graph: &mut GraphMiner) -> usize {
        let mut count = 0;
        for paper in &self.papers {
            let node = graph.add_paper(&paper.id, &paper.title);

            for tag in &paper.tags {
                graph.add_tag(&paper.id, node, tag);
            }

            if let Some(ref domain) = paper.domain {
                graph.add_tag(&paper.id, node, &format!("domain:{}", domain));
            }

            if let Some(ref insight) = paper.key_insight {
                graph.add_tag(&paper.id, node, &format!("insight:{}", insight));
            }

            count += 1;
        }
        info!("Importé {} papiers vers GraphMiner", count);
        count
    }

    /// Importe tous les papiers vers VectorStore.
    pub fn import_to_vector_store(&self, vs: &mut VectorStore, id_offset: usize) -> usize {
        let mut count = 0;
        for (i, paper) in self.papers.iter().enumerate() {
            let text = format!(
                "{} {} {}",
                paper.title,
                paper.abstract_text.as_deref().unwrap_or(""),
                paper.key_insight.as_deref().unwrap_or("")
            );
            vs.add(id_offset + i, &text);
            count += 1;
        }
        info!("Importé {} papiers vers VectorStore", count);
        count
    }

    /// Importe vers les 3 stores à la fois.
    pub fn import_all(
        &self,
        cognition: Option<&mut CognitionStore>,
        graph: Option<&mut GraphMiner>,
        vs: Option<(&mut VectorStore, usize)>,
    ) -> HashMap<String, usize> {
        let mut results = HashMap::new();
        if let Some(c) = cognition {
            results.insert("cognition".into(), self.import_to_cognition(c));
        }
        if let Some(g) = graph {
            results.insert("knowledge_graph".into(), self.import_to_graph(g));
        }
        if let Some((v, offset)) = vs {
            results.insert("vector_store".into(), self.import_to_vector_store(v, offset));
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_entry(id: &str) -> PaperEntry {
        PaperEntry {
            id: id.into(),
            title: format!("Paper {}", id),
            authors: vec!["Author A".into(), "Author B".into()],
            year: Some(2024),
            domain: Some("reinforcement_learning".into()),
            tags: vec!["rl".into(), "deep_learning".into()],
            abstract_text: Some("This is an abstract about reinforcement learning.".into()),
            key_insight: Some("Novel Q-learning variant.".into()),
            url: Some(format!("https://arxiv.org/abs/{}.00001", id)),
            source: Some("arXiv".into()),
            relevance_score: Some(0.85),
            github_url: None,
        }
    }

    #[test]
    fn test_add_and_get() {
        let dir = std::env::temp_dir().join("papers_test_registry");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_registry.json");
        let mut reg = PaperRegistry::new(&path);

        let entry = make_test_entry("TEST-001");
        reg.add(entry);
        assert_eq!(reg.len(), 1);

        let retrieved = reg.get("TEST-001");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Paper TEST-001");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search() {
        let mut reg = PaperRegistry::new("/tmp/papers_test_search.json");
        reg.add(make_test_entry("A"));
        reg.add(make_test_entry("B"));

        let results = reg.search("reinforcement");
        assert!(!results.is_empty(), "should find at least one paper");
    }

    #[test]
    fn test_filter_by_tag() {
        let mut reg = PaperRegistry::new("/tmp/papers_test_tag.json");
        reg.add(make_test_entry("X"));
        let results = reg.filter_by_tag("rl");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_stats() {
        let mut reg = PaperRegistry::new("/tmp/papers_test_stats.json");
        reg.add(make_test_entry("S1"));
        let stats = reg.stats();
        assert_eq!(stats.total_papers, 1);
        assert_eq!(stats.by_year.get(&2024), Some(&1));
    }
}
