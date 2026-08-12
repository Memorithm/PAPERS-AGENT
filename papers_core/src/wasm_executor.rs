use std::sync::Arc;
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

/// Result of genuine WASM execution, or an announced refusal when the input has
/// not actually been compiled to WASM.
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
/// Only [`Self::execute`] performs empirical execution. Rust source text is not
/// implicitly treated as WASM and is never assigned a synthetic execution score.
pub struct WasmExecutor {
    engine: Engine,
    config: WasmConfig,
    _ticker: Arc<()>,
}

impl WasmExecutor {
    pub fn new(config: WasmConfig) -> Result<Self> {
        let mut engine_config = wasmtime::Config::new();
        engine_config.epoch_interruption(true);
        engine_config.async_support(false);

        let engine = Engine::new(&engine_config)?;
        let engine_handle = engine.clone();
        let ticker = Arc::new(());
        let ticker_clone = ticker.clone();
        std::thread::Builder::new()
            .name("wasm-epoch-ticker".into())
            .spawn(move || {
                while Arc::strong_count(&ticker_clone) > 1 {
                    engine_handle.increment_epoch();
                    std::thread::sleep(Duration::from_millis(10));
                }
            })?;

        Ok(Self {
            engine,
            config,
            _ticker: ticker,
        })
    }

    /// Execute a genuine WASM binary exporting `main: () -> ()`.
    pub fn execute(&self, wasm_bytes: &[u8]) -> Result<WasmResult> {
        let start = Instant::now();

        let mut store = Store::new(&self.engine, ());
        if let Err(e) = store.set_fuel(self.config.fuel_limit) {
            return Ok(WasmResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to set fuel: {e}")),
                fuel_consumed: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }
        store.epoch_deadline_trap();
        let ticks = (self.config.timeout.as_millis() / 10).max(1) as u64;
        store.set_epoch_deadline(ticks);

        let module = match Module::new(&self.engine, wasm_bytes) {
            Ok(m) => m,
            Err(e) => {
                return Ok(WasmResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Compile error: {e}")),
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
                    error: Some(format!("Instantiation failed: {e}")),
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
                    error: Some(format!("No 'main' export found: {e}")),
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
                    error: Some(format!("Failed to read fuel: {e}")),
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
                    format!("Execution error: {e}")
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

    /// Refuse to pretend that Rust source text was executed.
    ///
    /// PAPERS may still generate Rust candidates, but empirical evaluation must
    /// be delegated to a real compiler/evaluator (RSI/CCOS Research Lab) or the
    /// caller must explicitly compile the program to WASM and call [`execute`].
    pub fn execute_rust_source(&self, source: &str) -> Result<WasmResult> {
        let reason = if !source.contains("fn ") {
            "Rust source rejected before execution: no function definition found"
        } else {
            "Rust source was not executed: compile it to WASM and call execute(), or delegate empirical evaluation to RSI/CCOS Research Lab"
        };
        Ok(WasmResult {
            success: false,
            output: String::new(),
            error: Some(reason.into()),
            fuel_consumed: 0,
            duration_ms: 0,
        })
    }

    pub fn validate_wasm(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && bytes[0..4] == [0x00, 0x61, 0x73, 0x6D]
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        assert!(WasmExecutor::new(WasmConfig::default()).is_ok());
    }

    #[test]
    fn test_validate_wasm_valid() {
        let valid_wasm = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(WasmExecutor::validate_wasm(&valid_wasm));
    }

    #[test]
    fn test_validate_wasm_invalid() {
        assert!(!WasmExecutor::validate_wasm(&[0x00, 0x00, 0x00, 0x00]));
    }

    #[test]
    fn test_validate_wasm_too_short() {
        assert!(!WasmExecutor::validate_wasm(&[0x00, 0x61]));
    }

    #[test]
    fn test_execute_invalid_wasm() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let result = executor
            .execute(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00])
            .unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_execute_rust_source_no_fn_is_refused() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let result = executor.execute_rust_source("let x = 5;").unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("no function"));
    }

    #[test]
    fn test_execute_rust_source_with_main_is_still_not_execution() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let result = executor
            .execute_rust_source("fn main() { println!(\"hello\"); }")
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.fuel_consumed, 0);
        assert!(result.error.unwrap().contains("was not executed"));
    }

    #[test]
    fn test_config_defaults() {
        let cfg = WasmConfig::default();
        assert_eq!(cfg.fuel_limit, 1_000_000);
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }
}
