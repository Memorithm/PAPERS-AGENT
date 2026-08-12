use statrs::distribution::{ContinuousCDF, StudentsT};

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

    /// Log-unnormalized posterior: log(P(D|H) * P(H)).
    fn log_posterior(&self) -> f64 {
        let log_prior = self.prior.max(1e-15).ln();
        if self.likelihoods.is_empty() {
            return log_prior;
        }
        let log_likelihood: f64 = self.likelihoods.iter().map(|l| l.max(1e-15).ln()).sum();
        log_prior + log_likelihood
    }

    /// Standalone evidence score mapped to (0, 1).
    ///
    /// This is not a normalized probability across competing hypotheses. Use
    /// [`ProbabilisticReasoner::compare`] when comparing hypotheses in the same
    /// reasoner.
    pub fn posterior(&self) -> f64 {
        let lp = self.log_posterior();
        if lp >= 0.0 {
            1.0 / (1.0 + (-lp).exp())
        } else {
            let exp_lp = lp.exp();
            exp_lp / (1.0 + exp_lp)
        }
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
            default_prior: default_prior.clamp(1e-15, 1.0 - 1e-15),
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
            .max_by(|a, b| a.log_posterior().total_cmp(&b.log_posterior()))
    }

    /// Posterior odds P(H1|D) / P(H2|D) under the reasoner's normalized
    /// softmax over log-posteriors. Returns 1.0 when either id is unknown.
    pub fn compare(&self, id1: &str, id2: &str) -> f64 {
        let idx1 = self.hypotheses.iter().position(|h| h.id == id1);
        let idx2 = self.hypotheses.iter().position(|h| h.id == id2);
        match (idx1, idx2) {
            (Some(idx1), Some(idx2)) => {
                let log_posts: Vec<f64> = self
                    .hypotheses
                    .iter()
                    .map(Hypothesis::log_posterior)
                    .collect();
                let max_lp = log_posts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let exp_posts: Vec<f64> = log_posts.iter().map(|lp| (lp - max_lp).exp()).collect();
                let sum_exp: f64 = exp_posts.iter().sum();
                if !sum_exp.is_finite() || sum_exp <= f64::MIN_POSITIVE {
                    return 1.0;
                }
                let p1 = exp_posts[idx1] / sum_exp;
                let p2 = exp_posts[idx2] / sum_exp;
                if p2 <= f64::MIN_POSITIVE {
                    f64::INFINITY
                } else {
                    p1 / p2
                }
            }
            _ => 1.0,
        }
    }

    /// Two-sided 95% confidence interval for the mean observed likelihood.
    ///
    /// Uses Student's t distribution because these sample sets are typically
    /// small. The previous implementation returned CDF probabilities at the
    /// interval endpoints, which were not interval bounds at all.
    pub fn uncertainty(&self, id: &str) -> Option<(f64, f64)> {
        let h = self.hypotheses.iter().find(|h| h.id == id)?;
        if h.likelihoods.len() < 2 {
            return None;
        }

        let n = h.likelihoods.len() as f64;
        let mean = h.likelihoods.iter().sum::<f64>() / n;
        let variance = h
            .likelihoods
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        let sem = variance.sqrt() / n.sqrt();

        if sem == 0.0 {
            return Some((mean, mean));
        }

        let student = StudentsT::new(0.0, 1.0, n - 1.0).ok()?;
        let critical = student.inverse_cdf(0.975);
        let margin = critical * sem;
        Some(((mean - margin).max(0.0), (mean + margin).min(1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertainty_returns_interval_bounds_not_cdf_values() {
        let mut reasoner = ProbabilisticReasoner::new(0.5);
        reasoner.register("h", "hypothesis", None);
        for value in [0.70, 0.75, 0.80, 0.85] {
            reasoner.observe("h", value);
        }

        let (low, high) = reasoner.uncertainty("h").expect("interval");
        let mean = 0.775;
        assert!(low < mean && high > mean);
        assert!((0.0..=1.0).contains(&low));
        assert!((0.0..=1.0).contains(&high));
    }

    #[test]
    fn zero_variance_interval_collapses_to_mean() {
        let mut reasoner = ProbabilisticReasoner::new(0.5);
        reasoner.register("h", "hypothesis", None);
        reasoner.observe("h", 0.8);
        reasoner.observe("h", 0.8);
        assert_eq!(reasoner.uncertainty("h"), Some((0.8, 0.8)));
    }

    #[test]
    fn comparison_is_stable_for_extreme_evidence() {
        let mut reasoner = ProbabilisticReasoner::new(0.5);
        reasoner.register("a", "a", None);
        reasoner.register("b", "b", None);
        for _ in 0..1000 {
            reasoner.observe("a", 1.0 - 1e-12);
            reasoner.observe("b", 1e-12);
        }
        let odds = reasoner.compare("a", "b");
        assert!(odds.is_finite() || odds.is_infinite());
        assert!(odds >= 1.0);
    }
}
