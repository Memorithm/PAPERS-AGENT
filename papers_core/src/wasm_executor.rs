use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use wasmtime::*;

/// Cible de compilation pour l'exécution sandboxée.
const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Délai maximal d'une invocation rustc pour la compilation candidat→WASM.
const RUSTC_TIMEOUT_SECS: u64 = 60;

/// Échecs possibles de la compilation Rust → WASM.
#[derive(Debug, Clone)]
pub enum CompileToWasmError {
    /// Le binaire `rustc` n'est pas disponible dans l'environnement.
    RustcUnavailable,
    /// La cible `wasm32-unknown-unknown` n'est pas installée
    /// (`rustup target add wasm32-unknown-unknown`).
    WasmTargetMissing,
    /// Aucun point d'entrée explicite compatible avec l'ABI PAPERS n'a été trouvé.
    EntrypointMissing,
    /// Erreur de compilation du candidat (extrait stderr inclus).
    Compilation(String),
    /// Erreur d'E/S locale (tempdir, lecture/écriture).
    Io(String),
}

impl CompileToWasmError {
    fn message(&self) -> String {
        match self {
            Self::RustcUnavailable => {
                "rustc introuvable : impossible de compiler le candidat vers WASM".into()
            }
            Self::WasmTargetMissing => {
                "cible wasm32-unknown-unknown absente : exécutez `rustup target add \
                 wasm32-unknown-unknown` pour activer l'exécution réelle des candidats"
                    .into()
            }
            Self::EntrypointMissing => {
                "candidat refusé : export `main: () -> ()` ou fonction `run()` explicite requis"
                    .into()
            }
            Self::Compilation(msg) => msg.clone(),
            Self::Io(msg) => format!("E/S compilation WASM: {msg}"),
        }
    }
}

impl std::fmt::Display for CompileToWasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Génère l'adaptateur `main` selon le contenu du candidat.
fn adapter_for(source: &str) -> std::result::Result<&'static str, CompileToWasmError> {
    let has_exported_main = source.contains("no_mangle")
        && source.contains("extern \"C\"")
        && (source.contains("fn main(") || source.contains("fn main ()"));
    if has_exported_main {
        return Ok("");
    }
    if source.contains("fn run(") || source.contains("fn run ()") {
        // Candidat conforme à la convention `run()` : appel réel.
        Ok("\n// Adaptateur généré par PAPERS : exposition de run() comme point d'entrée.\n#[no_mangle]\npub extern \"C\" fn main() {\n    run();\n}\n")
    } else {
        // Un helper qui compile n'est pas une exécution de l'algorithme annoncé.
        Err(CompileToWasmError::EntrypointMissing)
    }
}

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
        // Requis pour que Store::set_fuel fonctionne : sans cette option,
        // toute exécution réelle échouait avec "fuel is not configured".
        engine_config.consume_fuel(true);

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
    /// caller must explicitly compile the program to WASM and call [`Self::execute`].
    pub fn execute_rust_source(&self, source: &str) -> Result<WasmResult> {
        let start = Instant::now();
        if !source.contains("fn ") {
            return Ok(WasmResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Rust source rejected before execution: no function definition found".into(),
                ),
                fuel_consumed: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        match self.compile_source_to_wasm(source) {
            Ok(wasm_bytes) => self.execute(&wasm_bytes),
            Err(e) => Ok(WasmResult {
                success: false,
                output: String::new(),
                error: Some(e.message()),
                fuel_consumed: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    /// Compile réellement un source Rust vers WASM (`wasm32-unknown-unknown`)
    /// pour exécution sandboxée par [`Self::execute`].
    ///
    /// Convention d'entrée :
    /// - le source exporte déjà `#[no_mangle] pub extern "C" fn main()` → il
    ///   est compilé tel quel ;
    /// - le source définit `fn run()` (zéro arg) → un adaptateur généré appelle
    ///   `run` depuis `main` ;
    /// - sinon → la compilation est refusée : PAPERS ne fabrique jamais un
    ///   `main` vide pour transformer un helper en exécution réussie.
    pub fn compile_source_to_wasm(
        &self,
        source: &str,
    ) -> std::result::Result<Vec<u8>, CompileToWasmError> {
        let dir =
            tempfile::tempdir().map_err(|e| CompileToWasmError::Io(format!("tempdir: {e}")))?;
        let src_path = dir.path().join("candidate.rs");
        let adapter = adapter_for(source)?;
        let full_source = format!("{}\n{}", source.trim(), adapter);
        std::fs::write(&src_path, full_source)
            .map_err(|e| CompileToWasmError::Io(format!("écriture source: {e}")))?;

        let output_path = dir.path().join("candidate.wasm");
        let child = Command::new("rustc")
            .arg("--target")
            .arg(WASM_TARGET)
            .arg("--crate-type=cdylib")
            .arg("-C")
            .arg("opt-level=0")
            .arg("-o")
            .arg(&output_path)
            .arg(&src_path)
            .stderr(Stdio::piped())
            .spawn();

        let mut child = match child {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(CompileToWasmError::RustcUnavailable);
            }
            Err(e) => return Err(CompileToWasmError::Io(format!("lancement rustc: {e}"))),
        };

        // Attente bornée pour ne jamais bloquer le moteur sur un rustc récalcitrant.
        let deadline = Instant::now() + Duration::from_secs(RUSTC_TIMEOUT_SECS);
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(CompileToWasmError::Compilation(format!(
                        "rustc a dépassé le délai de {RUSTC_TIMEOUT_SECS}s"
                    )));
                }
                Err(e) => return Err(CompileToWasmError::Io(format!("attente rustc: {e}"))),
            }
        };
        let Some(status) = status else {
            return Err(CompileToWasmError::Io("statut rustc inconnu".into()));
        };

        if !status.success() {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                use std::io::Read;
                let _ = pipe.read_to_string(&mut stderr);
            }
            if stderr.contains("E0463")
                || stderr.to_lowercase().contains("target")
                    && stderr.to_lowercase().contains("not installed")
            {
                return Err(CompileToWasmError::WasmTargetMissing);
            }
            let preview: String = stderr.chars().take(500).collect();
            return Err(CompileToWasmError::Compilation(
                if preview.trim().is_empty() {
                    format!("rustc a échoué ({})", status)
                } else {
                    format!("Compilation échouée: {}", preview.trim_end())
                },
            ));
        }

        let bytes = std::fs::read(&output_path)
            .map_err(|e| CompileToWasmError::Io(format!("lecture WASM: {e}")))?;
        if !Self::validate_wasm(&bytes) {
            return Err(CompileToWasmError::Io(
                "sortie rustc sans en-tête WASM valide".into(),
            ));
        }
        Ok(bytes)
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

    /// Indique si la chaîne d'outils wasm32 est disponible dans cet environnement.
    fn wasm_toolchain_available() -> bool {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        executor
            .compile_source_to_wasm("pub fn run() {}")
            .is_ok()
    }

    #[test]
    fn test_execute_rust_source_compiles_and_runs_run_convention() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let source = r#"
pub fn compute() -> u64 { 6 * 7 }

pub fn run() {
    // Computation réelle exécutée dans le sandbox ; le résultat n'est pas
    // observable hors du module mais l'exécution consomme du fuel.
    let _ = compute();
}
"#;
        let result = executor.execute_rust_source(source).unwrap();
        if wasm_toolchain_available() {
            assert!(
                result.success,
                "exécution réelle attendue: {:?}",
                result.error
            );
            assert!(result.fuel_consumed > 0, "du fuel doit être consommé");
            assert!(result.duration_ms <= executor.config.timeout.as_millis() as u64 + 5_000);
        } else {
            assert!(!result.success);
            let err = result.error.unwrap_or_default();
            assert!(
                err.contains("wasm32") || err.contains("rustc"),
                "message d'indication attendu, obtenu: {err}"
            );
        }
    }

    #[test]
    fn test_execute_rust_source_rejects_type_error() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let source = r#"
pub fn run() {
    let x: i32 = "pas un nombre";
    let _ = x;
}
"#;
        let result = executor.execute_rust_source(source).unwrap();
        assert!(!result.success);
        match result.error {
            Some(err) if err.contains("Compilation échouée") => {}
            Some(err) if err.contains("wasm32") || err.contains("rustc") => {}
            other => panic!("erreur de compilation attendue, obtenu: {other:?}"),
        }
    }

    #[test]
    fn test_compile_source_to_wasm_produces_valid_module() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        let bytes = executor.compile_source_to_wasm("pub fn run() { let _ = 1_u8; }");
        match bytes {
            Ok(wasm) => {
                assert!(WasmExecutor::validate_wasm(&wasm));
                // L'adaptateur a bien généré un export main : exécution réussie.
                let result = executor.execute(&wasm).unwrap();
                assert!(result.success, "{:?}", result.error);
            }
            Err(CompileToWasmError::RustcUnavailable)
            | Err(CompileToWasmError::WasmTargetMissing) => {}
            Err(other) => panic!("échec inattendu: {other}"),
        }
    }

    #[test]
    fn test_adapter_selection() {
        // main déjà exporté → pas d'adaptateur.
        let exported = "#[no_mangle]\npub extern \"C\" fn main() {}";
        assert_eq!(adapter_for(exported).unwrap(), "");
        // Convention run() → adaptateur appelant run.
        assert!(adapter_for("pub fn run() {}").unwrap().contains("run();"));
        // Sinon → refus explicite, jamais de `main` vide.
        assert!(matches!(
            adapter_for("pub fn foo() {}"),
            Err(CompileToWasmError::EntrypointMissing)
        ));
    }

    #[test]
    fn test_missing_entrypoint_is_rejected_before_toolchain_lookup() {
        let executor = WasmExecutor::new(WasmConfig::default()).unwrap();
        assert!(matches!(
            executor.compile_source_to_wasm("pub fn helper() -> u8 { 1 }"),
            Err(CompileToWasmError::EntrypointMissing)
        ));
    }

    #[test]
    fn test_config_defaults() {
        let cfg = WasmConfig::default();
        assert_eq!(cfg.fuel_limit, 1_000_000);
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }
}
