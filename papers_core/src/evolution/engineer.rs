use std::collections::HashMap;
use std::time::Instant;

use crate::wasm_executor::{WasmConfig, WasmExecutor, WasmResult};

/// Authority of an evaluation result.
///
/// Only an explicit task oracle can establish empirical fitness. Runtime
/// execution and structural analysis remain useful diagnostics, but must never
/// promote a generated candidate by themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationAuthority {
    TaskOracle,
    WasmRuntimeDiagnostic,
    StructuralDiagnostic,
}

impl EvaluationAuthority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaskOracle => "task_oracle",
            Self::WasmRuntimeDiagnostic => "wasm_runtime_diagnostic",
            Self::StructuralDiagnostic => "structural_diagnostic",
        }
    }
}

/// Result of evaluating a candidate program.
#[derive(Debug, Clone)]
pub struct EngineerOutput {
    pub success: bool,
    /// Score emitted by the selected evaluation stage.
    ///
    /// This is empirical fitness only when `authority == TaskOracle`.
    pub score: f64,
    pub error: Option<String>,
    pub metrics: HashMap<String, f64>,
    pub runtime_secs: f64,
    pub authority: EvaluationAuthority,
}

impl EngineerOutput {
    #[must_use]
    pub const fn is_empirically_validated(&self) -> bool {
        self.success && matches!(self.authority, EvaluationAuthority::TaskOracle)
    }
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
        Self {
            timeout_secs,
            wasm: None,
        }
    }

    /// Attach a WasmExecutor for sandboxed evaluation.
    pub fn with_wasm(mut self, config: WasmConfig) -> Self {
        let wasm = WasmExecutor::new(config).ok();
        self.wasm = wasm;
        self
    }

    /// Evaluate a program using the injected `eval_fn`, WASM, or structural analysis.
    pub fn execute<F>(&self, program: &str, eval_fn: Option<F>) -> EngineerOutput
    where
        F: FnOnce(&str) -> EngineerOutput,
    {
        let start = Instant::now();

        if let Some(f) = eval_fn {
            let mut result = f(program);
            // Supplying the evaluator is the explicit task-oracle boundary.
            result.authority = EvaluationAuthority::TaskOracle;
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
                        authority: EvaluationAuthority::WasmRuntimeDiagnostic,
                    };
                }
            }
        }

        // Fallback: structural analysis with code quality metrics
        self.structural_analysis(program, start)
    }

    /// Convert a WasmResult to EngineerOutput.
    fn wasm_result_to_output(&self, wasm: &WasmResult, start: Instant) -> EngineerOutput {
        let mut metrics = HashMap::new();
        metrics.insert("fuel_consumed".into(), wasm.fuel_consumed as f64);
        metrics.insert("duration_ms".into(), wasm.duration_ms as f64);

        metrics.insert(
            "runtime_success".into(),
            if wasm.success { 1.0 } else { 0.0 },
        );

        EngineerOutput {
            success: wasm.success,
            // Runtime success is not task correctness. Fuel is evidence about
            // work consumed, never a positive reward signal.
            score: 0.0,
            error: wasm.error.clone(),
            metrics,
            runtime_secs: start.elapsed().as_secs_f64(),
            authority: EvaluationAuthority::WasmRuntimeDiagnostic,
        }
    }

    /// Detailed structural analysis of Rust program with code quality scoring.
    fn structural_analysis(&self, program: &str, start: Instant) -> EngineerOutput {
        let mut metrics = HashMap::new();
        let lines = program.lines().count() as f64;
        metrics.insert("lines".into(), lines);

        let fn_count = program.matches("fn ").count();
        let struct_count = program.matches("struct ").count();
        let impl_count = program.matches("impl ").count();
        let enum_count = program.matches("enum ").count();
        let trait_count = program.matches("trait ").count();
        metrics.insert("functions".into(), fn_count as f64);
        metrics.insert("structs".into(), struct_count as f64);
        metrics.insert("impls".into(), impl_count as f64);
        metrics.insert("enums".into(), enum_count as f64);
        metrics.insert("traits".into(), trait_count as f64);

        // Control flow metrics
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

        // Quality indicators
        let has_return_types = program.contains("-> ");
        let has_doc_comments = program.contains("///");
        let has_unsafe = program.contains("unsafe ");
        let has_tests = program.contains("#[test]");
        let has_error_handling = program.contains("Result<") || program.contains("Option<");
        let has_trait_impls = impl_count > 0 && trait_count > 0;

        let has_fn = fn_count > 0;
        let has_braces = program.contains('{') && program.contains('}');
        let valid = has_fn && has_braces;

        if valid {
            let completeness = if has_return_types { 0.15 } else { 0.0 }
                + if has_doc_comments { 0.1 } else { 0.0 }
                + if has_error_handling { 0.1 } else { 0.0 }
                + if !has_unsafe { 0.05 } else { 0.0 };
            let complexity = (lines / 80.0).min(1.0) * 0.2;
            let diversity =
                ((fn_count + struct_count + enum_count + trait_count) as f64 / 5.0).min(1.0) * 0.2;
            let quality =
                if has_tests { 0.1 } else { 0.0 } + if has_trait_impls { 0.05 } else { 0.0 };
            let score = 0.2 + completeness + complexity + diversity + quality;

            EngineerOutput {
                success: true,
                score,
                error: None,
                metrics,
                runtime_secs: start.elapsed().as_secs_f64(),
                authority: EvaluationAuthority::StructuralDiagnostic,
            }
        } else {
            let mut error = Vec::new();
            if !has_fn {
                error.push("No function definition found");
            }
            if !has_braces {
                error.push("Missing braces");
            }
            EngineerOutput {
                success: false,
                score: 0.1,
                error: Some(error.join("; ")),
                metrics,
                runtime_secs: start.elapsed().as_secs_f64(),
                authority: EvaluationAuthority::StructuralDiagnostic,
            }
        }
    }

    /// Combined health-check + execution (backward-compatible).
    pub fn execute_simple(&self, program: &str) -> (bool, f64) {
        let has_rust = program.contains("fn ")
            || program.contains("struct ")
            || program.contains("impl ")
            || program.contains("use ");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_runtime_success_is_diagnostic_not_empirical_fitness() {
        let engineer = Engineer::new(1);
        let result = engineer.wasm_result_to_output(
            &WasmResult {
                success: true,
                output: String::new(),
                error: None,
                fuel_consumed: 900_000,
                duration_ms: 12,
            },
            Instant::now(),
        );
        assert!(result.success);
        assert_eq!(result.authority, EvaluationAuthority::WasmRuntimeDiagnostic);
        assert_eq!(result.score, 0.0);
        assert!(!result.is_empirically_validated());
        assert_eq!(result.metrics.get("fuel_consumed"), Some(&900_000.0));
    }

    #[test]
    fn explicit_evaluator_is_the_task_oracle_boundary() {
        let engineer = Engineer::new(1);
        let output = engineer.execute(
            "pub fn run() {}",
            Some(|_: &str| EngineerOutput {
                success: true,
                score: 0.75,
                error: None,
                metrics: HashMap::new(),
                runtime_secs: 0.0,
                authority: EvaluationAuthority::StructuralDiagnostic,
            }),
        );
        assert_eq!(output.authority, EvaluationAuthority::TaskOracle);
        assert!(output.is_empirically_validated());
        assert_eq!(output.score, 0.75);
    }
}
