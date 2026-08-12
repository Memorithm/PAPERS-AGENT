/// PAPERS-side description of an architecture candidate.
///
/// `fitness` is retained for interchange compatibility, but PAPERS core no
/// longer fabricates it. Architecture search/evaluation belongs to SciRust or
/// CCOS Research Lab where candidates can be measured against a real workload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredArchitecture {
    pub layers: Vec<String>,
    pub fitness: f64,
    pub params_m: f64,
    pub flops: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArchSearchConfig {
    pub min_layers: usize,
    pub max_layers: usize,
    pub min_hidden: usize,
    pub max_hidden: usize,
    pub max_models: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArchSearch {
    config: ArchSearchConfig,
}

impl ArchSearch {
    pub fn new(max_layers: usize, max_hidden: usize) -> Self {
        Self {
            config: ArchSearchConfig {
                min_layers: 1,
                max_layers: max_layers.max(1),
                min_hidden: 16,
                max_hidden: max_hidden.max(16),
                max_models: 100,
                seed: 42,
            },
        }
    }

    pub fn config(&self) -> &ArchSearchConfig {
        &self.config
    }

    /// Fail closed instead of assigning synthetic NAS fitness.
    ///
    /// The previous implementation directly linked a machine-local SciRust
    /// checkout via `/tmp/scirust`. That made PAPERS unreproducible and blurred
    /// the responsibility boundary: PAPERS proposes scientific hypotheses;
    /// SciRust/CCOS Research Lab performs architecture search and empirical
    /// evaluation. Consumers should serialize `config()` and invoke that runtime.
    pub fn evolve(
        &mut self,
        _generations: usize,
        _pop_size: usize,
    ) -> Result<Vec<DiscoveredArchitecture>, String> {
        Err(
            "NAS evaluation is not available inside PAPERS core: delegate to the SciRust/CCOS Research Lab evaluator; no synthetic fitness was produced"
                .into(),
        )
    }

    pub fn explore(&mut self, n: usize) -> Result<Vec<DiscoveredArchitecture>, String> {
        self.evolve(1, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_deterministic() {
        let a = ArchSearch::new(8, 512);
        let b = ArchSearch::new(8, 512);
        assert_eq!(a.config().seed, b.config().seed);
        assert_eq!(a.config().max_layers, 8);
        assert_eq!(a.config().max_hidden, 512);
    }

    #[test]
    fn search_refuses_to_invent_fitness() {
        let mut search = ArchSearch::new(4, 128);
        let error = search.evolve(10, 32).unwrap_err();
        assert!(error.contains("no synthetic fitness"));
    }
}
