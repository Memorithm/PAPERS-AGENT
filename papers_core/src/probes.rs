use std::time::Instant;

/// Benchmark probe for measuring code performance and quality.
pub struct RustProbe {
    program: String,
    /// CPU time limit for benchmarks in seconds
    cpu_limit: f64,
    /// Memory limit for benchmarks in MB
    memory_limit: f64,
}

impl RustProbe {
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            cpu_limit: 10.0,     // Default 10 second CPU limit
            memory_limit: 512.0, // Default 512 MB memory limit
        }
    }

    pub fn run_tests(&self) -> Result<ProbeResult, String> {
        let mut test_results = Vec::new();
        let start_time = Instant::now();

        // Test 1: Compilation test
        let compilation_result = self.test_compilation();
        test_results.push(("compilation", compilation_result));

        // Test 2: Performance test
        let performance_result = self.test_performance();
        test_results.push(("performance", performance_result));

        // Test 3: Memory usage test
        let memory_result = self.test_memory_usage();
        test_results.push(("memory", memory_result));

        // Test 4: Code complexity test
        let complexity_result = self.test_complexity();
        test_results.push(("complexity", complexity_result));

        // Test 5: Security test
        let security_result = self.test_security();
        test_results.push(("security", security_result));

        let total_time = start_time.elapsed().as_secs_f64();

        if total_time > self.cpu_limit {
            return Err(format!(
                "Benchmark exceeded CPU limit of {}s",
                self.cpu_limit
            ));
        }

        let mut passed = 0;
        let mut total_tests = 0;
        let mut errors = Vec::new();

        for (name, result) in &test_results {
            match result {
                Ok(success) => {
                    total_tests += 1;
                    if *success {
                        passed += 1;
                    } else {
                        errors.push(format!("Test '{}' failed", name));
                    }
                }
                Err(e) => {
                    errors.push(format!("Test '{}' error: {}", name, e));
                }
            }
        }

        let success_rate = if total_tests > 0 {
            passed as f64 / total_tests as f64
        } else {
            0.0
        };

        Ok(ProbeResult {
            success_rate,
            total_tests,
            passed,
            errors,
            execution_time: total_time,
            test_details: test_results,
        })
    }

    /// Test that the code compiles without errors.
    fn test_compilation(&self) -> Result<bool, String> {
        // This is a simplified compilation test
        // In a real implementation, this would actually compile the code
        if self.program.is_empty() {
            return Err("Program is empty".to_string());
        }

        // Check for basic Rust syntax
        if !self.validate_basic_syntax() {
            return Err("Basic syntax validation failed".to_string());
        }

        // Check for required constructs
        if !self.has_required_constructs() {
            return Err("Missing required constructs".to_string());
        }

        Ok(true)
    }

    /// Validate basic Rust syntax.
    fn validate_basic_syntax(&self) -> bool {
        let lines = self.program.lines();
        let mut brace_count = 0;
        let mut paren_count = 0;
        let mut in_string = false;
        let mut in_block_comment = false;

        for line in lines {
            let mut in_line_comment = false;
            let chars: Vec<char> = line.chars().collect();
            let len = chars.len();

            for i in 0..len {
                let ch = chars[i];

                if in_string {
                    if ch == '\\' && i + 1 < len {
                        continue; // skip escaped char
                    }
                    if ch == '"' {
                        in_string = false;
                    }
                    continue;
                }

                if in_line_comment {
                    continue;
                }

                if in_block_comment {
                    if ch == '*' && i + 1 < len && chars[i + 1] == '/' {
                        in_block_comment = false;
                    }
                    continue;
                }

                match ch {
                    '"' => in_string = true,
                    '/' if i + 1 < len && chars[i + 1] == '/' => {
                        in_line_comment = true;
                    }
                    '/' if i + 1 < len && chars[i + 1] == '*' => {
                        in_block_comment = true;
                    }
                    '#' if i == 0 => {
                        // Line starts with # — attribute or cfg, skip rest
                        in_line_comment = true;
                    }
                    '{' => brace_count += 1,
                    '}' => {
                        if brace_count == 0 {
                            return false;
                        }
                        brace_count -= 1;
                    }
                    '(' => paren_count += 1,
                    ')' => {
                        if paren_count == 0 {
                            return false;
                        }
                        paren_count -= 1;
                    }
                    _ => {}
                }
            }
        }

        brace_count == 0 && paren_count == 0 && !in_string && !in_block_comment
    }

    /// Check for required constructs in valid Rust code.
    fn has_required_constructs(&self) -> bool {
        let has_fn = self.program.contains("fn ") || self.program.contains("main()");
        let has_braces = self.program.contains('{') && self.program.contains('}');

        has_fn && has_braces
    }

    /// Test code performance.
    fn test_performance(&self) -> Result<bool, String> {
        // This would run the compiled code and measure performance
        // For now, do a simple static analysis
        let complexity = self.calculate_complexity();
        let estimated_runtime = self.estimate_runtime();

        if estimated_runtime > self.cpu_limit {
            return Err(format!(
                "Estimated runtime {}s exceeds limit",
                estimated_runtime
            ));
        }

        if complexity > 10000.0 {
            return Err("Code complexity too high".to_string());
        }

        Ok(true)
    }

    /// Calculate code complexity.
    fn calculate_complexity(&self) -> f64 {
        let mut complexity = 0.0;

        for line in self.program.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue; // Skip comments
            }

            // Count control flow statements
            if trimmed.contains("if ") {
                complexity += 5.0;
            }
            if trimmed.contains("for ") {
                complexity += 10.0;
            }
            if trimmed.contains("while ") {
                complexity += 15.0;
            }
            if trimmed.contains("match ") {
                complexity += 20.0;
            }
            if trimmed.contains("loop { ") {
                complexity += 25.0;
            }

            // Count function calls
            let call_count = trimmed.matches('.').count() + trimmed.matches('(').count()
                - trimmed.matches(')').count();
            complexity += call_count as f64 * 2.0;

            // Count lines of code
            complexity += 1.0;
        }

        complexity
    }

    /// Estimate runtime of the code.
    fn estimate_runtime(&self) -> f64 {
        // Rough estimate based on program size
        let lines = self.program.lines().count() as f64;
        let control_flow_complexity = self.calculate_complexity();

        // Base multiplier for line count
        let line_multiplier = lines * 0.01;

        // Add complexity-based multiplier
        let complexity_multiplier = control_flow_complexity / 1000.0;

        line_multiplier + complexity_multiplier
    }

    /// Test memory usage.
    fn test_memory_usage(&self) -> Result<bool, String> {
        let estimated_memory = self.estimate_memory_usage();

        if estimated_memory > self.memory_limit {
            return Err(format!(
                "Estimated memory usage {}MB exceeds limit",
                estimated_memory
            ));
        }

        Ok(true)
    }

    /// Estimate memory usage of the code.
    fn estimate_memory_usage(&self) -> f64 {
        // Rough estimate based on data structures and allocations
        let mut memory = 0.0;

        for line in self.program.lines() {
            let trimmed = line.trim();

            // Vector allocations
            if trimmed.contains("vec![") {
                memory += 100.0;
            }

            // HashMap allocations
            if trimmed.contains("HashMap::new()") || trimmed.contains(".insert(") {
                memory += 50.0;
            }

            // String allocations
            if trimmed.contains("String::") || trimmed.contains("format!(") {
                memory += 10.0;
            }

            // Large allocations
            if trimmed.contains("Box::new") || trimmed.contains("Arc::new") {
                memory += 200.0;
            }
        }

        // Base memory for program size
        memory += (self.program.len() as f64) * 0.01;

        memory
    }

    /// Test code security.
    fn test_security(&self) -> Result<bool, String> {
        let mut issues = Vec::new();

        if self.program.contains("unsafe {") {
            issues.push("Unsafe block detected".to_string());
        }

        if self.program.contains("std::process::Command") && self.program.contains(".arg(") {
            issues.push("Shell command execution detected".to_string());
        }

        if self.program.contains("std::fs::remove") {
            issues.push("File deletion detected".to_string());
        }

        if !issues.is_empty() {
            return Err(format!("Security issues: {:?}", issues));
        }

        Ok(true)
    }

    /// Test code complexity.
    fn test_complexity(&self) -> Result<bool, String> {
        let complexity = self.calculate_complexity();
        let cyclomatic_complexity = self.calculate_cyclomatic_complexity();

        if complexity > 5000.0 {
            return Err(format!("Code complexity too high: {:.1}", complexity));
        }

        if cyclomatic_complexity > 50 {
            return Err(format!(
                "Cyclomatic complexity too high: {}",
                cyclomatic_complexity
            ));
        }

        Ok(true)
    }

    /// Calculate cyclomatic complexity.
    fn calculate_cyclomatic_complexity(&self) -> usize {
        let mut complexity = 1; // Base complexity
        let mut in_match = false;

        for line in self.program.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Count decision points
            if trimmed.contains("if ") {
                complexity += 1;
            }
            if trimmed.contains("for ") {
                complexity += 1;
            }
            if trimmed.contains("while ") {
                complexity += 1;
            }
            if trimmed.contains("match ") {
                in_match = true;
            }
            if in_match {
                complexity += trimmed.matches("=>").count();
                // End match block on a standalone closing brace (not on an arm line)
                if trimmed == "}" || trimmed.starts_with("} //") {
                    in_match = false;
                }
            }
            if trimmed.contains("&&") {
                complexity += 1;
            }
            if trimmed.contains("||") {
                complexity += 1;
            }
        }

        complexity
    }
}

/// Result of a comprehensive benchmark test.
pub struct ProbeResult {
    pub success_rate: f64,
    pub total_tests: usize,
    pub passed: usize,
    pub errors: Vec<String>,
    pub execution_time: f64,
    pub test_details: Vec<(&'static str, Result<bool, String>)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_basic_syntax_valid() {
        let program = r#"
fn main() {
    let x = 42;
    println!("{}", x);
}
"#;
        let probe = RustProbe::new(program);
        assert!(probe.validate_basic_syntax());
    }

    #[test]
    fn test_validate_basic_syntax_invalid_unmatched_brace() {
        let program = r#"
fn main() {
    let x = 42;
"#;
        let probe = RustProbe::new(program);
        assert!(!probe.validate_basic_syntax());
    }

    #[test]
    fn test_validate_basic_syntax_invalid_unmatched_paren() {
        let program = r#"
fn main() {
    let x = (1 + 2;
}
"#;
        let probe = RustProbe::new(program);
        assert!(!probe.validate_basic_syntax());
    }

    #[test]
    fn test_has_required_constructs() {
        let probe = RustProbe::new("fn foo() { let x = 1; }");
        assert!(probe.has_required_constructs());

        let probe = RustProbe::new("let x = 1;");
        assert!(!probe.has_required_constructs());
    }

    #[test]
    fn test_calculate_cyclomatic_complexity() {
        let program = r#"
fn main() {
    if true {
        if false {
        }
    }
    for i in 0..10 {
        while i < 5 {
            match i {
                1 => {},
                2 => {},
            }
        }
    }
}
"#;
        let probe = RustProbe::new(program);
        let cc = probe.calculate_cyclomatic_complexity();
        assert_eq!(cc, 7);
    }

    #[test]
    fn test_estimate_memory_usage() {
        let program = r#"
fn main() {
    let v = vec![1, 2, 3];
    let s = String::from("hello");
    let m = HashMap::new();
}
"#;
        let probe = RustProbe::new(program);
        let mem = probe.estimate_memory_usage();
        assert!(mem > 0.0);
    }

    #[test]
    fn test_run_tests_valid_program() {
        let program = r#"
fn main() {
    let x = 1 + 1;
    println!("{}", x);
}
"#;
        let probe = RustProbe::new(program);
        let result = probe.run_tests();
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success_rate > 0.0);
        assert!(result.passed > 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_run_tests_empty_program() {
        let probe = RustProbe::new("");
        let result = probe.run_tests();
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.errors.is_empty());
    }
}
