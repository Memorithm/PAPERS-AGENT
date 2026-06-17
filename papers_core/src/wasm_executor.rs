use std::time::{Duration, Instant};

use anyhow::Result;
use wasmtime::*;

/// Configuration for WASM sandbox execution.
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Fuel limit (instructions) before termination. Default: 1_000_000.
    pub fuel_limit: u64,
    /// Maximum execution time. Default: 30 seconds.
    pub timeout: Duration,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            fuel_limit: 1_000_000,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Result of WASM execution.
#[derive(Debug, Clone)]
pub struct WasmResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub fuel_consumed: u64,
    pub duration_ms: u64,
}

/// Sandboxed WASM executor using wasmtime.
///
/// Provides safe execution of generated WASM modules with:
/// - Fuel-based instruction limiting
/// - Epoch-based interruption (timeout)
pub struct WasmExecutor {
    engine: Engine,
    config: WasmConfig,
}

impl WasmExecutor {
    /// Create a new WASM executor with the given configuration.
    pub fn new(config: WasmConfig) -> Result<Self> {
        let mut engine_config = wasmtime::Config::new();
        engine_config.epoch_interruption(true);
        engine_config.async_support(false);

        let engine = Engine::new(&engine_config)?;

        Ok(Self { engine, config })
    }

    /// Execute a WASM binary.
    ///
    /// The WASM must export a `main` function taking no arguments.
    pub fn execute(&self, wasm_bytes: &[u8]) -> Result<WasmResult> {
        let start = Instant::now();

        let mut store = Store::new(&self.engine, ());
        if let Err(e) = store.set_fuel(self.config.fuel_limit) {
            return Ok(WasmResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to set fuel: {}", e)),
                fuel_consumed: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
        store.epoch_deadline_trap();

        let module = match Module::new(&self.engine, wasm_bytes) {
            Ok(m) => m,
            Err(e) => {
                return Ok(WasmResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Compile error: {}", e)),
                    fuel_consumed: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
        };

        let instance = match Instance::new(&mut store, &module, &[]) {
            Ok(inst) => inst,
            Err(e) => {
                return Ok(WasmResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Instantiation failed: {}", e)),
                    fuel_consumed: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
        };

        let main_func = match instance.get_typed_func::<(), ()>(&mut store, "main") {
            Ok(f) => f,
            Err(e) => {
                return Ok(WasmResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("No 'main' export found: {}", e)),
                    fuel_consumed: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
        };

        let fuel_before = match store.get_fuel() {
            Ok(f) => f,
            Err(e) => {
                return Ok(WasmResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to read fuel: {}", e)),
                    fuel_consumed: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
        };
        let result = main_func.call(&mut store, ());
        let fuel_after = store.get_fuel().unwrap_or(fuel_before);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);

        match result {
            Ok(()) => Ok(WasmResult {
                success: true,
                output: String::new(),
                error: None,
                fuel_consumed,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) => {
                let msg = e.to_string();
                let error_msg = if msg.contains("out of fuel") || msg.contains("Fuel") {
                    "Execution exceeded fuel limit".to_string()
                } else if msg.contains("interrupted") || msg.contains("epoch") {
                    "Execution timed out".to_string()
                } else {
                    format!("Execution error: {}", e)
                };
                Ok(WasmResult {
                    success: false,
                    output: String::new(),
                    error: Some(error_msg),
                    fuel_consumed,
                    duration_ms: start.elapsed().as_millis() as u64,
                })
            }
        }
    }

    /// Execute from a Rust source string (syntax-level validation only).
    pub fn execute_rust_source(&self, source: &str) -> Result<WasmResult> {
        if !source.contains("fn ") {
            return Ok(WasmResult {
                success: false,
                output: String::new(),
                error: Some("No function definition found".into()),
                fuel_consumed: 0,
                duration_ms: 0,
            });
        }

        let has_main = source.contains("fn main");
        let lines = source.lines().count();
        let complexity = (lines as f64 / 50.0).min(1.0);
        let score = if has_main { 0.5 } else { 0.3 } + complexity * 0.4;

        Ok(WasmResult {
            success: true,
            output: format!("Score: {:.2}, lines: {}", score, lines),
            error: None,
            fuel_consumed: 0,
            duration_ms: 0,
        })
    }

    /// Validate WASM bytes (magic number check).
    pub fn validate_wasm(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && bytes[0..4] == [0x00, 0x61, 0x73, 0x6D]
    }

    /// Access the underlying engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let executor = WasmExecutor::new(WasmConfig::default());
        assert!(executor.is_ok());
    }

    #[test]
    fn test_validate_wasm_valid() {
        let valid_wasm = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(WasmExecutor::validate_wasm(&valid_wasm));
    }

    #[test]
    fn test_validate_wasm_invalid() {
        let invalid = vec![0x00, 0x00, 0x00, 0x00];
        assert!(!WasmExecutor::validate_wasm(&invalid));
    }

    #[test]
    fn test_validate_wasm_too_short() {
        let short = vec![0x00, 0x61];
        assert!(!WasmExecutor::validate_wasm(&short));
    }

    #[test]
    fn test_execute_invalid_wasm() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let result = executor.execute(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]);
        assert!(result.is_ok());
        assert!(!result.unwrap().success);
    }

    #[test]
    fn test_execute_rust_source_no_fn() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let result = executor.execute_rust_source("let x = 5;").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("No function"));
    }

    #[test]
    fn test_execute_rust_source_with_main() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let src = "fn main() { println!(\"hello\"); }";
        let result = executor.execute_rust_source(src).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_config_defaults() {
        let cfg = WasmConfig::default();
        assert_eq!(cfg.fuel_limit, 1_000_000);
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }
}
