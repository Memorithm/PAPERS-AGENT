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
    /// Vrai si le fichier principal n'a pas pu être chargé (absence de données fiables).
    degraded: bool,
}

impl PaperRegistry {
    /// Charge ou crée un registre au chemin donné.
    ///
    /// En cas de fichier corrompu ou illisible, tente la restauration depuis
    /// le backup `<chemin>.bak`. Si aucune source n'est exploitable, le
    /// registre démarre vide en mode dégradé : [`save`](Self::save) refusera
    /// alors d'écraser les données disque pour éviter toute perte.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut degraded = false;
        let mut papers = Vec::new();

        if path.exists() {
            match fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|content| {
                    serde_json::from_str::<Vec<PaperEntry>>(&content).map_err(|e| e.to_string())
                }) {
                Ok(loaded) => papers = loaded,
                Err(e) => {
                    warn!("PaperRegistry corrompu ({}): {}", path.display(), e);
                    degraded = true;
                    let backup = Self::backup_path(&path);
                    if backup.exists() {
                        match fs::read_to_string(&backup)
                            .map_err(|e| e.to_string())
                            .and_then(|content| {
                                serde_json::from_str::<Vec<PaperEntry>>(&content)
                                    .map_err(|e| e.to_string())
                            }) {
                            Ok(restored) => {
                                papers = restored;
                                warn!(
                                    "PaperRegistry restauré depuis {}: {} papiers",
                                    backup.display(),
                                    papers.len()
                                );
                            }
                            Err(be) => warn!("Backup illisible ({}): {}", backup.display(), be),
                        }
                    } else {
                        warn!(
                            "Aucun backup disponible ({}); registre démarré à vide en mode dégradé",
                            backup.display()
                        );
                    }
                }
            }
        }
        info!("PaperRegistry chargé: {} papiers", papers.len());
        Self {
            papers,
            path,
            degraded,
        }
    }

    fn backup_path(path: &Path) -> std::path::PathBuf {
        let mut name = path
            .file_name()
            .map(|s| s.to_os_string())
            .unwrap_or_default();
        name.push(".bak");
        path.with_file_name(name)
    }

    /// Indique si le registre a été chargé en mode dégradé (fichier corrompu).
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Sauvegarde le registre sur le disque.
    ///
    /// Écriture atomique (fichier temporaire + rename) ; l'ancienne version est
    /// conservée dans `<chemin>.bak`. Refuse d'écraser un registre corrompu
    /// avec un état vide afin de ne jamais détruire les données existantes.
    pub fn save(&self) -> Result<(), String> {
        if self.degraded && self.papers.is_empty() {
            return Err(format!(
                "Refus de sauvegarder un registre vide par-dessus le fichier corrompu {} \
                 (données préservées). Restaurez ou supprimez manuellement le fichier.",
                self.path.display()
            ));
        }
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| format!("Création dossier: {}", e))?;
            }
        }
        let json = serde_json::to_string_pretty(&self.papers)
            .map_err(|e| format!("Sérialisation: {}", e))?;

        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, &json).map_err(|e| format!("Écriture temporaire: {}", e))?;

        if self.path.exists() {
            let backup = Self::backup_path(&self.path);
            fs::copy(&self.path, &backup).map_err(|e| format!("Copie backup: {}", e))?;
        }
        fs::rename(&tmp, &self.path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Renommage atomique: {}", e)
        })?;
        info!("PaperRegistry sauvegardé: {} papiers", self.papers.len());
        Ok(())
    }

    /// Force l'écrasement du fichier même si le registre est en mode dégradé
    /// et vide (l'appelant assume la perte des anciennes données).
    pub fn force_save(&mut self) -> Result<(), String> {
        self.degraded = false;
        self.save()
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
                format!(
                    "Key Insight: {}",
                    paper.key_insight.as_deref().unwrap_or("")
                ),
                paper.abstract_text.as_deref().unwrap_or("").to_string(),
            ]
            .join("\n\n");

            store.add(CognitionItem::new(
                content,
                paper.source.clone().unwrap_or_else(|| paper.title.clone()),
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
            results.insert(
                "vector_store".into(),
                self.import_to_vector_store(v, offset),
            );
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

    #[test]
    fn test_save_creates_backup_and_roundtrips() {
        let dir = std::env::temp_dir().join(format!("papers_reg_bak_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("registry.json");

        let mut reg = PaperRegistry::new(&path);
        assert!(!reg.is_degraded());
        reg.add(make_test_entry("B1"));
        reg.save().expect("first save");

        reg.add(make_test_entry("B2"));
        reg.save().expect("second save");

        // Le backup contient l'état précédent (1 papier).
        let bak = dir.join("registry.json.bak");
        assert!(bak.exists(), "backup should exist after second save");
        let backup: Vec<PaperEntry> =
            serde_json::from_str(&fs::read_to_string(&bak).unwrap()).unwrap();
        assert_eq!(backup.len(), 1);
        assert_eq!(backup[0].id, "B1");

        // Rechargement : les 2 papiers sont présents.
        let reloaded = PaperRegistry::new(&path);
        assert_eq!(reloaded.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupted_registry_restores_from_backup() {
        let dir = std::env::temp_dir().join(format!("papers_reg_corrupt_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("registry.json");

        // État sain : deux sauvegardes successives => .bak contient l'état précédent.
        let mut reg = PaperRegistry::new(&path);
        reg.add(make_test_entry("R1"));
        reg.save().unwrap();
        reg.add(make_test_entry("R2"));
        reg.save().unwrap();

        // Corruption du fichier principal.
        fs::write(&path, "%%%pas du json%%%").unwrap();

        let recovered = PaperRegistry::new(&path);
        assert!(
            recovered.is_degraded(),
            "corruption must mark degraded mode"
        );
        assert_eq!(recovered.len(), 1, "must fall back to previous .bak state");
        assert!(recovered.get("R1").is_some());

        // La re-sauvegarde est autorisée car les données ont été récupérées.
        recovered.save().unwrap();
        assert_eq!(PaperRegistry::new(&path).len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_refuses_to_wipe_corrupted_registry() {
        let dir = std::env::temp_dir().join(format!("papers_refuse_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("registry.json");
        fs::write(&path, "{corrompu sans backup").unwrap();

        let mut reg = PaperRegistry::new(&path);
        assert!(
            reg.is_degraded(),
            "unparseable file must mark degraded mode"
        );
        assert!(reg.is_empty());

        let err = reg.save().expect_err("save must refuse to wipe data");
        assert!(err.contains("Refus"), "unexpected error: {err}");
        // Le fichier d'origine est intact sur disque (aucun écrasement).
        let on_disk = fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("corrompu"));

        // force_save assume explicitement la perte.
        reg.force_save().unwrap();
        assert!(!reg.is_degraded());
        assert!(PaperRegistry::new(&path).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
