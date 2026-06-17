use scirust_nas::{NasSearch, NasConfig};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArchSearch {
    config: NasConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredArchitecture {
    pub layers: Vec<String>,
    pub fitness: f64,
    pub params_m: f64,
    pub flops: f64,
}

impl ArchSearch {
    pub fn new(max_layers: usize, max_hidden: usize) -> Self {
        let config = NasConfig {
            min_layers: 1,
            max_layers,
            min_hidden: 16,
            max_hidden,
            accuracy_weight: 1.0,
            params_weight: -0.01,
            flops_weight: -0.001,
            max_models: 100,
            seed: 42,
        };
        Self { config }
    }

    /// Evolve architectures
    pub fn evolve(
        &mut self,
        generations: usize,
        pop_size: usize,
    ) -> Result<Vec<DiscoveredArchitecture>, String> {
        let mut search = NasSearch::new(self.config.clone());
        let results = search.evolve(generations, pop_size)
            .map_err(|e| format!("NAS search failed: {}", e))?;
        Ok(results.into_iter().map(|arch| {
            DiscoveredArchitecture {
                layers: arch.layers.iter().map(|l| format!("{}", l)).collect(),
                fitness: arch.fitness,
                params_m: arch.params_m,
                flops: arch.flops,
            }
        }).collect())
    }

    /// Quick single-generation search
    pub fn explore(&mut self, n: usize) -> Result<Vec<DiscoveredArchitecture>, String> {
        self.evolve(1, n)
    }
}
