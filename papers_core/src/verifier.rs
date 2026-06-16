#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationReport {
    pub candidate_id: String,
    pub passed: bool,
    pub score: f64,
    pub counterexamples: Vec<String>,
    pub violations: Vec<String>,
    pub suggestions: Vec<String>,
}

pub struct Verifier;

impl Verifier {
    pub fn new() -> Self {
        Self
    }

    pub fn verify(
        &self,
        candidate_id: &str,
        program: &str,
        success: bool,
        error: Option<&str>,
    ) -> VerificationReport {
        let mut counterexamples = Vec::new();
        let mut violations = Vec::new();
        let mut suggestions = Vec::new();

        if let Some(err) = error {
            if !err.is_empty() {
                counterexamples.push(format!("Runtime error: {}", &err[..err.len().min(200)]));
                suggestions.push("Fix the runtime error before resubmitting.".into());
            }
        }

        if !success {
            violations.push("Program execution failed.".into());
        }

        let has_function = program.contains("def ");
        let has_imports = program.contains("import ");
        if !has_function {
            violations.push("Program does not define any function.".into());
            suggestions.push("Add a function definition.".into());
        }
        if !has_imports {
            suggestions.push("Consider adding necessary imports.".into());
        }

        let complexity = program.lines().filter(|l| l.contains("if ") || l.contains("for ") || l.contains("while ")).count();
        if complexity > 50 {
            suggestions.push("Program is very complex ({} branches). Consider simplifying.".into());
        }

        let non_deterministic = ["random.", "np.random", "time.time", "uuid."]
            .iter()
            .any(|nd| program.contains(nd));
        if non_deterministic {
            suggestions.push("Program contains non-deterministic operations. Consider seeding for reproducibility.".into());
        }

        let passed = counterexamples.is_empty() && violations.is_empty();
        let score = if passed { 1.0 } else { 0.5 - 0.1 * counterexamples.len() as f64 - 0.1 * violations.len() as f64 };

        VerificationReport {
            candidate_id: candidate_id.into(),
            passed,
            score: score.max(0.0),
            counterexamples,
            violations,
            suggestions,
        }
    }
}
