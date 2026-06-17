use std::collections::HashMap;
use std::time::Instant;

use crate::wasm_executor::{WasmExecutor, WasmConfig, WasmResult};

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
/// Uses `WasmExecutor` for sandboxed evaluation when available, or performs
/// structural analysis with code quality metrics as a fallback.
pub struct Engineer {
    #[allow(dead_code)]
    timeout_secs: u64,
    #[allow(dead_code)]
    wasm: Option<WasmExecutor>,
}

impl Engineer {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs, wasm: None }
    }

    /// Attach a WasmExecutor for sandboxed evaluation.
    pub fn with_wasm(mut self, config: WasmConfig) -> Self {
        let wasm = WasmExecutor::new(config).ok();
        self.wasm = wasm;
        self
    }

    /// Evaluate a program using the injected `eval_fn` or built-in analysis.
    pub fn execute<F>(&self, program: &str, eval_fn: Option<F>) -> EngineerOutput
    where
        F: FnOnce(&str) -> EngineerOutput,
    {
        let start = Instant::now();

        // If an eval_fn is provided, use it
        if let Some(f) = eval_fn {
            let mut result = f(program);
            result.runtime_secs = start.elapsed().as_secs_f64();
            return result;
        }

        // Try WASM evaluation if executor is available
        if let Some(ref wasm) = self.wasm {
            let result = wasm.execute_rust_source(program);
            match result {
                Ok(wasm_result) => {
                    return self.wasm_result_to_output(&wasm_result, start);
                }
                Err(e) => {
                    let mut metrics = HashMap::new();
                    metrics.insert("lines".into(), program.lines().count() as f64);
                    return EngineerOutput {
                        success: false,
                        score: 0.0,
                        error: Some(format!("WASM error: {}", e)),
                        metrics,
                        runtime_secs: start.elapsed().as_secs_f64(),
                    };
                }
            }
        }

        // Default: structural analysis with code quality metrics
        self.structural_analysis(program, start)
    }

    /// Convert a WasmResult to EngineerOutput.
    fn wasm_result_to_output(&self, wasm: &WasmResult, start: Instant) -> EngineerOutput {
        let mut metrics = HashMap::new();
        metrics.insert("fuel_consumed".into(), wasm.fuel_consumed as f64);
        metrics.insert("duration_ms".into(), wasm.duration_ms as f64);

        // Score based on complexity metrics and success
        let base = if wasm.success { 0.4 } else { 0.1 };
        let fuel_efficiency = (wasm.fuel_consumed as f64 / 1000.0).min(1.0) * 0.3;

        EngineerOutput {
            success: wasm.success,
            score: base + fuel_efficiency,
            error: wasm.error.clone(),
            metrics,
            runtime_secs: start.elapsed().as_secs_f64(),
        }
    }

    /// Detailed structural analysis of Rust program.
    fn structural_analysis(&self, program: &str, start: Instant) -> EngineerOutput {
        let mut metrics = HashMap::new();
        let lines = program.lines().count() as f64;
        metrics.insert("lines".into(), lines);

        // Count functions
        let fn_count = program.matches("fn ").count();
        metrics.insert("functions".into(), fn_count as f64);

        // Count structs, impls, enums
        let struct_count = program.matches("struct ").count();
        let impl_count = program.matches("impl ").count();
        let _enum_count = program.matches("enum ").count();
        metrics.insert("structs".into(), struct_count as f64);
        metrics.insert("impls".into(), impl_count as f64);

        // Control flow complexity: if/else/for/while/loop/match
        let control_flow = [
            ("if ", "if_count"),
            ("for ", "for_count"),
            ("while ", "while_count"),
            ("match ", "match_count"),
            ("loop ", "loop_count"),
        ];
        for (pat, key) in &control_flow {
            metrics.insert(key.to_string(), program.matches(pat).count() as f64);
        }

        // Return types indicate function completeness
        let has_return_types = program.contains("-> ");
        let has_doc_comments = program.contains("///");
        let has_unsafe = program.contains("unsafe ");

        // Scoring
        let has_fn = fn_count > 0;
        let has_braces = program.contains('{') && program.contains('}');
        let valid = has_fn && has_braces;

        if valid {
            let completeness = if has_return_types { 0.2 } else { 0.0 }
                + if has_doc_comments { 0.1 } else { 0.0 }
                + if !has_unsafe { 0.1 } else { 0.0 };
            let complexity = (lines / 50.0).min(1.0) * 0.3;
            let diversity = (fn_count as f64 / 3.0).min(1.0) * 0.2;
            let score = 0.2 + completeness + complexity + diversity;

            EngineerOutput {
                success: true,
                score,
                error: None,
                metrics,
                runtime_secs: start.elapsed().as_secs_f64(),
            }
        } else {
            let mut error = Vec::new();
            if !has_fn { error.push("No function definition found"); }
            if !has_braces { error.push("Missing braces"); }
            EngineerOutput {
                success: false,
                score: 0.1,
                error: Some(error.join("; ")),
                metrics,
                runtime_secs: start.elapsed().as_secs_f64(),
            }
        }
    }

    /// Combined health-check + execution (backward-compatible).
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
