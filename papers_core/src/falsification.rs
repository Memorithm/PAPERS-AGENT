#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FalsificationTest {
    pub test_id: String,
    pub description: String,
    pub hypothesis_id: String,
    pub adversarial: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FalsificationResult {
    pub test: FalsificationTest,
    pub passed: bool,
    pub actual: String,
}

pub struct FalsificationEngine {
    pub tests: Vec<FalsificationTest>,
}

impl Default for FalsificationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FalsificationEngine {
    pub fn new() -> Self {
        Self { tests: Vec::new() }
    }

    pub fn generate_tests(&mut self, hypothesis_id: &str, code: &str, text: &str) {
        let boundary_tests = vec![
            ("empty", "What happens with empty input?"),
            ("zero", "What happens with zero values?"),
            ("negative", "What happens with negative values?"),
            ("none", "What happens with missing parameters?"),
        ];

        for (name, desc) in boundary_tests {
            self.tests.push(FalsificationTest {
                test_id: format!("{}_{}", hypothesis_id, name),
                description: desc.into(),
                hypothesis_id: hypothesis_id.into(),
                adversarial: false,
            });
        }

        for keyword in &["always", "never", "guaranteed", "optimal", "best"] {
            if text.to_lowercase().contains(keyword) {
                self.tests.push(FalsificationTest {
                    test_id: format!("{}_adv_{}", hypothesis_id, keyword),
                    description: format!(
                        "Falsification test: claim uses '{}' - find counterexample",
                        keyword
                    ),
                    hypothesis_id: hypothesis_id.into(),
                    adversarial: true,
                });
            }
        }

        let complexity = code
            .lines()
            .filter(|l| l.contains("if ") || l.contains("for ") || l.contains("while "))
            .count();
        if complexity == 0 {
            self.tests.push(FalsificationTest {
                test_id: format!("{}_complexity", hypothesis_id),
                description: "Program has no control flow - likely trivial solution".into(),
                hypothesis_id: hypothesis_id.into(),
                adversarial: true,
            });
        }
    }

    pub fn compute_falsifiability(
        &self,
        hypothesis_id: &str,
        results: &[FalsificationResult],
    ) -> f64 {
        let relevant: Vec<&FalsificationResult> = results
            .iter()
            .filter(|r| r.test.hypothesis_id == hypothesis_id)
            .collect();
        if relevant.is_empty() {
            return 0.5;
        }
        let passed = relevant.iter().filter(|r| r.passed).count() as f64;
        1.0 - (passed / relevant.len() as f64)
    }

    pub fn detect_reward_hacking(&self, score: f64, falsifiability: f64) -> String {
        let ratio = falsifiability / score.max(1e-6);
        if ratio > 2.0 {
            "HIGH".into()
        } else if ratio > 1.0 {
            "MEDIUM".into()
        } else {
            "LOW".into()
        }
    }
}
