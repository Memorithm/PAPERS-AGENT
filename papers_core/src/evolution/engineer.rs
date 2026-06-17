use std::collections::HashMap;
use std::time::Instant;

/// Result of evaluating a candidate program.
pub struct EngineerOutput {
    pub success: bool,
    pub score: f64,
    pub error: Option<String>,
    pub metrics: HashMap<String, f64>,
    pub runtime_secs: f64,
}

/// Evaluates candidate programs produced by the Researcher.
///
/// In the Python version, this executed programs via subprocess and measured
/// fitness. In Rust, the evaluation function is injected as a closure since
/// we cannot safely compile and run arbitrary Rust code at runtime (that
/// requires a sandboxed WASM engine or similar).
pub struct Engineer {
    #[allow(dead_code)]
    timeout_secs: u64,
}

impl Engineer {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Evaluate a program using the injected `eval_fn`.
    ///
    /// `eval_fn` receives the program text and returns (success, score, error).
    /// If `eval_fn` is None, the program is checked for structural validity
    /// (has functions, no obvious defects) and scored syntactically.
    pub fn execute<F>(&self, program: &str, eval_fn: Option<F>) -> EngineerOutput
    where
        F: FnOnce(&str) -> EngineerOutput,
    {
        let start = Instant::now();

        if let Some(f) = eval_fn {
            let mut result = f(program);
            result.runtime_secs = start.elapsed().as_secs_f64();
            return result;
        }

        // Default: check structural validity
        let has_fn = program.contains("fn ");
        let has_braces = program.contains('{') && program.contains('}');

        let valid = has_fn && has_braces;
        let complexity = program.lines().count().min(50) as f64 / 50.0;
        let score = if valid { 0.3 + complexity * 0.5 } else { 0.1 };

        let mut metrics = HashMap::new();
        metrics.insert("lines".into(), program.lines().count() as f64);
        metrics.insert("complexity".into(), complexity);

        let error = if !valid {
            if !has_fn { Some("No function definition found".into()) }
            else if !has_braces { Some("Missing braces".into()) }
            else { None }
        } else {
            None
        };

        EngineerOutput {
            success: valid,
            score,
            error,
            metrics,
            runtime_secs: start.elapsed().as_secs_f64(),
        }
    }

    /// Combined health-check + execution (backward-compatible with original EvolutionLoop).
    pub fn execute_simple(&self, program: &str) -> (bool, f64) {
        let has_rust = program.contains("fn ") || program.contains("struct ")
            || program.contains("impl ") || program.contains("use ");
        let has_braces = program.contains('{') && program.contains('}');
        let valid = has_rust && has_braces;
        let score = if valid {
            let lines = program.lines().count() as f64;
            let complexity = lines.min(50.0) / 50.0;
            0.3 + complexity * 0.5
        } else {
            0.1
        };
        (valid, score)
    }

    /// Compute combined fitness from metrics and optional LLM judge score.
    pub fn compute_fitness(
        metrics: &HashMap<String, f64>,
        primary_score: f64,
        llm_judge: Option<f64>,
    ) -> f64 {
        let primary = metrics.get("score").copied().unwrap_or(primary_score);
        match llm_judge {
            Some(judge) => primary * 0.8 + judge * 0.2,
            None => primary,
        }
    }
}
