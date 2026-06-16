use statrs::distribution::{ContinuousCDF, Normal};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub description: String,
    pub prior: f64,
    pub likelihoods: Vec<f64>,
}

impl Hypothesis {
    pub fn new(id: &str, description: &str, prior: f64) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            prior,
            likelihoods: Vec::new(),
        }
    }

    pub fn posterior(&self) -> f64 {
        if self.likelihoods.is_empty() {
            return self.prior;
        }
        let log_prior = self.prior.max(1e-15).ln();
        let log_likelihood: f64 = self.likelihoods.iter().map(|l| l.max(1e-15).ln()).sum();
        let total = log_prior + log_likelihood;
        1.0 / (1.0 + (-total).exp())
    }

    pub fn update(&mut self, likelihood: f64) {
        self.likelihoods.push(likelihood.clamp(1e-10, 1.0 - 1e-10));
    }
}

pub struct ProbabilisticReasoner {
    pub hypotheses: Vec<Hypothesis>,
    default_prior: f64,
}

impl ProbabilisticReasoner {
    pub fn new(default_prior: f64) -> Self {
        Self {
            hypotheses: Vec::new(),
            default_prior,
        }
    }

    pub fn register(&mut self, id: &str, description: &str, prior: Option<f64>) {
        self.hypotheses.push(Hypothesis::new(
            id,
            description,
            prior.unwrap_or(self.default_prior),
        ));
    }

    pub fn observe(&mut self, id: &str, strength: f64) {
        if let Some(h) = self.hypotheses.iter_mut().find(|h| h.id == id) {
            h.update(strength);
        } else {
            let mut h = Hypothesis::new(id, &format!("Auto: {}", id), self.default_prior);
            h.update(strength);
            self.hypotheses.push(h);
        }
    }

    pub fn best(&self) -> Option<&Hypothesis> {
        self.hypotheses
            .iter()
            .max_by(|a, b| a.posterior().partial_cmp(&b.posterior()).unwrap())
    }

    pub fn compare(&self, id1: &str, id2: &str) -> f64 {
        let h1 = self.hypotheses.iter().find(|h| h.id == id1);
        let h2 = self.hypotheses.iter().find(|h| h.id == id2);
        match (h1, h2) {
            (Some(a), Some(b)) => {
                let p1 = a.posterior().max(1e-15);
                let p2 = b.posterior().max(1e-15);
                p1 / p2
            }
            _ => 1.0,
        }
    }

    pub fn uncertainty(&self, id: &str) -> Option<(f64, f64)> {
        let h = self.hypotheses.iter().find(|h| h.id == id)?;
        if h.likelihoods.len() < 2 {
            return None;
        }
        let n = h.likelihoods.len() as f64;
        let mean = h.likelihoods.iter().sum::<f64>() / n;
        let variance = h.likelihoods.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std = variance.sqrt();
        let sem = std / n.sqrt();
        let normal = Normal::new(mean, sem).ok()?;
        Some((normal.cdf(mean - 1.96 * sem), normal.cdf(mean + 1.96 * sem)))
    }
}
