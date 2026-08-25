use std::path::Path;

use anyhow::{Context, Result};
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// Modèle LLM par défaut — source unique de vérité (CLI, config, client).
pub const DEFAULT_LLM_MODEL: &str = "gemma4:e2b";

/// URL Ollama par défaut — source unique de vérité.
pub const DEFAULT_LLM_BASE_URL: &str = "http://localhost:11434";

/// Configuration centralisée pour PAPERS V2.
///
/// Utilise `figment` pour la fusion de sources multiples :
/// 1. Valeurs par défaut
/// 2. Fichier `config.toml` (optionnel)
/// 3. Variables d'environnement avec préfixe `PAPERS_`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PapersConfig {
    pub evolution: EvolutionSection,
    pub llm: LlmSection,
    pub embedding: EmbeddingSection,
    pub wasm: WasmSection,
    pub logging: LoggingSection,
    #[serde(default)]
    pub gpu: GpuSection,
}

/// Section GPU : poids de scoring configurables (voir [`crate::gpu::GpuScoreWeights`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuSection {
    #[serde(default)]
    pub score_weights: crate::gpu::GpuScoreWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionSection {
    pub max_rounds: usize,
    pub sampling_policy: String,
    pub n_candidates_per_round: usize,
    pub n_context_nodes: usize,
    pub n_cognition: usize,
    pub patience: usize,
    pub target_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSection {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    /// Clé API (provider OpenAI-compatible). `None` pour Ollama local.
    pub api_key: Option<String>,
    pub timeout_secs: u64,
    /// Nombre de nouvelles tentatives sur erreur réseau/5xx (0 = pas de retry).
    pub retry_attempts: u32,
    /// Délai de base du backoff exponentiel entre tentatives (ms).
    pub retry_backoff_ms: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingSection {
    pub engine: String,
    pub model_path: Option<String>,
    pub dim: usize,
    pub hnsw_max_nb_connection: usize,
    pub hnsw_max_layer: usize,
    pub hnsw_ef_construction: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSection {
    pub fuel_limit: u64,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSection {
    pub level: String,
    pub format: String,
}

impl Default for PapersConfig {
    fn default() -> Self {
        Self {
            evolution: EvolutionSection {
                max_rounds: 50,
                sampling_policy: "greedy".into(),
                n_candidates_per_round: 3,
                n_context_nodes: 5,
                n_cognition: 5,
                patience: 10,
                target_score: None,
            },
            llm: LlmSection {
                provider: "ollama".into(),
                model: DEFAULT_LLM_MODEL.into(),
                base_url: DEFAULT_LLM_BASE_URL.into(),
                api_key: None,
                timeout_secs: 300,
                retry_attempts: 3,
                retry_backoff_ms: 500,
                max_tokens: 2048,
                temperature: 0.3,
            },
            embedding: EmbeddingSection {
                engine: "scirust".into(),
                model_path: None,
                dim: 128,
                hnsw_max_nb_connection: 16,
                hnsw_max_layer: 5,
                hnsw_ef_construction: 200,
            },
            wasm: WasmSection {
                fuel_limit: 1_000_000,
                timeout_secs: 30,
            },
            logging: LoggingSection {
                level: "info".into(),
                format: "text".into(),
            },
            gpu: GpuSection::default(),
        }
    }
}

impl PapersConfig {
    /// Charge la configuration depuis config.toml + env vars + defaults.
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut figment = Figment::from(Serialized::defaults(PapersConfig::default()));

        // Overlay from config.toml if it exists
        if let Some(path) = config_path {
            if path.exists() {
                figment = figment.merge(Toml::file(path));
            }
        } else {
            // Try default locations
            for candidate in &["config.toml", "papers.toml", ".papers/config.toml"] {
                let p = Path::new(candidate);
                if p.exists() {
                    figment = figment.merge(Toml::file(p));
                    break;
                }
            }
        }

        // Overlay from environment variables: PAPERS_EVOLUTION__MAX_ROUNDS=100
        figment = figment.merge(Env::prefixed("PAPERS_").split("__"));

        let config: PapersConfig = figment.extract().context("Failed to load configuration")?;

        Ok(config)
    }

    /// Sauvegarde la configuration en fichier TOML.
    pub fn save(&self, path: &Path) -> Result<()> {
        let toml = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(path, toml).context("Failed to write config file")?;
        Ok(())
    }

    /// Convertit en `EvolutionConfig` pour le moteur d'évolution.
    pub fn to_evolution_config(&self) -> crate::models::EvolutionConfig {
        crate::models::EvolutionConfig {
            task_description: String::new(),
            max_rounds: self.evolution.max_rounds,
            sampling_policy: self.evolution.sampling_policy.clone(),
            n_candidates_per_round: self.evolution.n_candidates_per_round,
            n_context_nodes: self.evolution.n_context_nodes,
            n_cognition: self.evolution.n_cognition,
            patience: self.evolution.patience,
            target_score: self.evolution.target_score,
        }
    }

    /// Convertit en `LlmConfig` pour le client LLM.
    pub fn to_llm_config(&self) -> crate::llm::LlmConfig {
        crate::llm::LlmConfig {
            provider: self.llm.provider.clone(),
            model: self.llm.model.clone(),
            base_url: self.llm.base_url.clone(),
            api_key: self.llm.api_key.clone(),
            timeout_secs: self.llm.timeout_secs,
            retry_attempts: self.llm.retry_attempts,
            retry_backoff_ms: self.llm.retry_backoff_ms,
            max_tokens: self.llm.max_tokens,
            temperature: self.llm.temperature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PapersConfig::default();
        assert_eq!(config.evolution.max_rounds, 50);
        assert_eq!(config.llm.model, DEFAULT_LLM_MODEL);
        assert_eq!(
            config.llm.api_key, None,
            "pas de clé API fantôme par défaut"
        );
        assert!(config.llm.retry_attempts >= 1);
        assert_eq!(config.embedding.dim, 128);
        assert_eq!(config.wasm.fuel_limit, 1_000_000);
    }

    #[test]
    fn test_load_config_no_file() {
        let config = PapersConfig::load(None);
        assert!(config.is_ok());
    }

    #[test]
    fn test_to_evolution_config() {
        let config = PapersConfig::default();
        let evo = config.to_evolution_config();
        assert_eq!(evo.max_rounds, 50);
        assert_eq!(evo.n_candidates_per_round, 3);
    }

    #[test]
    fn test_to_llm_config() {
        let config = PapersConfig::default();
        let llm = config.to_llm_config();
        assert_eq!(llm.model, DEFAULT_LLM_MODEL);
    }

    #[test]
    fn test_gpu_weights_configurable() {
        // Défauts : poids historiques.
        let config = PapersConfig::default();
        assert!((config.gpu.score_weights.training.vendor_bonus - 10.0).abs() < 1e-9);

        // Surcharge TOML de la section gpu (via figment, comme PapersConfig::load).
        let toml = r#"
[gpu.score_weights.inference]
memory_factor = 0.5
compute_high = 100.0
compute_mid = 50.0
compute_low = 10.0
compute_other = 1.0
vendor_bonus = 25.0
"#;
        let figment =
            Figment::from(Serialized::defaults(PapersConfig::default())).merge(Toml::string(toml));
        let parsed: PapersConfig = figment.extract().expect("merge TOML");
        assert!((parsed.gpu.score_weights.inference.memory_factor - 0.5).abs() < 1e-9);
        assert!((parsed.gpu.score_weights.inference.vendor_bonus - 25.0).abs() < 1e-9);
        // Les autres tâches gardent leurs défauts.
        assert!((parsed.gpu.score_weights.embedding.vendor_bonus - 3.0).abs() < 1e-9);
    }
}
