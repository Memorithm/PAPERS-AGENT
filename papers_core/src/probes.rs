use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Délai par défaut accordé à `rustc` pour compiler un programme de sonde.
const DEFAULT_COMPILE_TIMEOUT_SECS: u64 = 30;

/// Benchmark probe for measuring code performance and quality.
///
/// La compilation est réellement vérifiée via `rustc --emit=metadata` dans un
/// répertoire temporaire. Les tests de performance et mémoire restent des
/// estimations statiques (l'exécution arbitraire relève du sandbox WASM).
pub struct RustProbe {
    program: String,
    /// CPU time limit for benchmarks in seconds
    cpu_limit: f64,
    /// Memory limit for benchmarks in MB
    memory_limit: f64,
    /// Délai maximal d'une invocation rustc
    compile_timeout_secs: u64,
}

impl RustProbe {
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            cpu_limit: 10.0,     // Default 10 second CPU limit
            memory_limit: 512.0, // Default 512 MB memory limit
            compile_timeout_secs: DEFAULT_COMPILE_TIMEOUT_SECS,
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
    ///
    /// Pré-filtre structurel rapide, puis compilation réelle via `rustc
    /// --edition=2021 --emit=metadata` dans un répertoire temporaire. Si le
    /// binaire `rustc` est indisponible, repli documenté sur la vérification
    /// structurelle seule (jamais présentée comme une vraie compilation).
    fn test_compilation(&self) -> Result<bool, String> {
        if self.program.is_empty() {
            return Err("Program is empty".to_string());
        }

        if !self.validate_basic_syntax() {
            return Err("Basic syntax validation failed".to_string());
        }

        match self.compile_with_rustc() {
            CompilationOutcome::Success => Ok(true),
            CompilationOutcome::RustcUnavailable => {
                log::warn!(
                    "rustc introuvable : la sonde se limite à la vérification structurelle \
                     (aucune compilation réelle effectuée)"
                );
                if !self.has_required_constructs() {
                    return Err("Missing required constructs".to_string());
                }
                Ok(true)
            }
            CompilationOutcome::Failure(msg) => Err(msg),
        }
    }

    /// Compile réellement le programme avec rustc.
    ///
    /// Les warnings sont acceptés (`-A warnings`) : seul un échec de
    /// compilation est rejeté. Le stderr de rustc est capturé dans un fichier
    /// temporaire pour éviter tout deadlock de pipe et permettre l'affichage
    /// tronqué des erreurs.
    fn compile_with_rustc(&self) -> CompilationOutcome {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => return CompilationOutcome::Failure(format!("Création tempdir: {}", e)),
        };
        let src_path = dir.path().join("probe_program.rs");
        if let Err(e) = std::fs::write(&src_path, &self.program) {
            return CompilationOutcome::Failure(format!("Écriture source: {}", e));
        }
        let err_file = match tempfile::NamedTempFile::new() {
            Ok(f) => f,
            Err(e) => return CompilationOutcome::Failure(format!("Fichier stderr: {}", e)),
        };
        let err_handle = match err_file.reopen() {
            Ok(h) => h,
            Err(e) => return CompilationOutcome::Failure(format!("Réouverture stderr: {}", e)),
        };

        let child = Command::new("rustc")
            .arg("--edition=2021")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("-A")
            .arg("warnings")
            .arg("--out-dir")
            .arg(dir.path())
            .arg(&src_path)
            .stdout(Stdio::null())
            .stderr(Stdio::from(err_handle))
            .spawn();

        let mut child = match child {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return CompilationOutcome::RustcUnavailable;
            }
            Err(e) => return CompilationOutcome::Failure(format!("Lancement rustc: {}", e)),
        };

        // Attente avec délai : kill au-delà du timeout.
        let deadline = Instant::now() + Duration::from_secs(self.compile_timeout_secs);
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return CompilationOutcome::Failure(format!(
                        "rustc a dépassé le délai de {}s",
                        self.compile_timeout_secs
                    ));
                }
                Err(e) => return CompilationOutcome::Failure(format!("Attente rustc: {}", e)),
            }
        };

        let Some(status) = status else {
            return CompilationOutcome::Failure("rustc: statut inconnu".into());
        };

        if status.success() {
            return CompilationOutcome::Success;
        }

        // Récupère les erreurs rustc (tronquées) pour diagnostic.
        let mut stderr = String::new();
        if let Ok(mut f) = err_file.reopen() {
            let _ = f.read_to_string(&mut stderr);
        }
        let preview: String = stderr.chars().take(500).collect();
        CompilationOutcome::Failure(if preview.trim().is_empty() {
            format!("rustc a échoué ({})", status)
        } else {
            format!("Compilation échouée: {}", preview.trim_end())
        })
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

/// Issue d'une tentative de compilation réelle.
enum CompilationOutcome {
    Success,
    /// rustc absent de l'environnement (repli structurel documenté).
    RustcUnavailable,
    Failure(String),
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

    #[test]
    fn test_real_compilation_accepts_valid_rust() {
        let program = r#"
pub fn add(a: i64, b: i64) -> i64 { a + b }

pub fn build() -> Vec<String> {
    (0..3).map(|i| format!("item-{i}")).collect()
}
"#;
        let probe = RustProbe::new(program);
        let outcome = probe.compile_with_rustc();
        match outcome {
            CompilationOutcome::Success => {}
            CompilationOutcome::RustcUnavailable => {
                // Environnement sans rustc : la sonde doit replier proprement.
                assert!(probe.test_compilation().is_ok());
            }
            CompilationOutcome::Failure(e) => panic!("programme valide rejeté: {e}"),
        }
    }

    #[test]
    fn test_real_compilation_rejects_type_error() {
        let program = r#"
pub fn broken() -> i32 {
    let x: i32 = "pas un nombre";
    x
}
"#;
        let probe = RustProbe::new(program);
        match probe.compile_with_rustc() {
            CompilationOutcome::Failure(msg) => {
                assert!(msg.contains("Compilation échouée"), "msg: {msg}");
            }
            CompilationOutcome::RustcUnavailable => {}
            CompilationOutcome::Success => panic!("erreur de type acceptée par rustc ?!"),
        }
        // run_tests doit rapporter l'échec de compilation.
        let result = probe.run_tests().unwrap();
        assert!(
            result.errors.iter().any(|e| e.contains("compilation")),
            "erreurs: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_real_compilation_rejects_unresolved_name() {
        let program = r#"
pub fn calls_missing() -> u64 {
    fonction_qui_n_existe_pas(42)
}
"#;
        let probe = RustProbe::new(program);
        match probe.compile_with_rustc() {
            CompilationOutcome::Failure(_) => {}
            CompilationOutcome::RustcUnavailable => {}
            CompilationOutcome::Success => panic!("nom non résolu accepté ?!"),
        }
    }
}
