//! Typed PAPERS consumer for the qualified SciRust Hub gVisor boundary.
//! Qualification is rechecked by the exact Hub binary before every execution.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Context, Result};
use hub_executor::execution_guard::execute_guarded_command;
use hub_executor::gvisor::GvisorWorkspaceQuotaV1;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const REQUEST_SCHEMA_V1: &str = "scirust-hub.gvisor-execution-request/v1";
const RECEIPT_SCHEMA_V1: &str = "scirust-hub.gvisor-execution-receipt/v1";
const QUALIFICATION_SCHEMA_V1: &str = "scirust-hub.gvisor-qualification-evidence/v1";
const MAX_CAPTURE_BYTES: usize = 1024 * 1024;
const MAX_NATIVE_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct HubGvisorConfig {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
    /// Operator-provisioned bounded persistent volume below workspace_root.
    /// Only private task subdirectories are mounted into the guest.
    pub workspace_volume: PathBuf,
    pub workspace_quota: GvisorWorkspaceQuotaV1,
    pub qualification_manifest: PathBuf,
    pub qualification_receipt: PathBuf,
    pub image: String,
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub pids_limit: u64,
    /// Independent compilation budget; does not consume the execution budget.
    pub compile_timeout: Duration,
    /// Independent execution budget, beginning at execution-phase dispatch.
    pub timeout: Duration,
    pub max_source_bytes: usize,
    pub max_capture_bytes: usize,
}

impl HubGvisorConfig {
    fn validate(&self) -> Result<()> {
        let digest = self.image.split_once("@sha256:").map(|(_, value)| value);
        ensure!(
            digest.is_some_and(|value| lower_hex(value, 64)),
            "Hub gVisor image must use an immutable lowercase sha256 digest"
        );
        ensure!(self.cpu_millis > 0, "cpu_millis must be non-zero");
        ensure!(
            (1..=1024 * 1024 * 1024).contains(&self.workspace_quota.max_bytes)
                && (1..=65536).contains(&self.workspace_quota.max_inodes),
            "persistent workspace quota bounds are invalid"
        );
        ensure!(
            self.memory_bytes >= 16 * 1024 * 1024,
            "memory bound is too small"
        );
        ensure!(
            (1..=4096).contains(&self.pids_limit),
            "pids_limit must be between one and 4096"
        );
        for (name, timeout) in [
            ("compile_timeout", self.compile_timeout),
            ("timeout", self.timeout),
        ] {
            ensure!(
                (1..=3_600_000).contains(&timeout.as_millis()),
                "{name} must be between one millisecond and one hour"
            );
            ensure!(
                timeout
                    .checked_add(Duration::from_secs(15))
                    .and_then(|duration| Instant::now().checked_add(duration))
                    .is_some(),
                "{name} exceeds the outer deadline representation"
            );
        }
        ensure!(self.max_source_bytes > 0, "source bound must be non-zero");
        ensure!(
            (1..=MAX_CAPTURE_BYTES).contains(&self.max_capture_bytes),
            "capture bound is invalid"
        );
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HubGvisorResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub isolation_evidence_id: String,
    pub phase_evidence: Vec<HubGvisorPhaseEvidence>,
}

/// Selected immutable execution facts, not a scientific verdict or new task authority.
#[derive(Debug, Clone, Serialize)]
pub struct HubGvisorPhaseEvidence {
    pub phase: &'static str,
    pub task_id: String,
    pub timeout_ms: u64,
    pub source_sha256: String,
    pub artifact_sha256: Option<String>,
    pub request_sha256: String,
    pub receipt_sha256: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub isolation_evidence_id: String,
}

/// A refusal retains only already verified phases; it never attests the refused phase.
#[derive(Debug)]
pub struct HubGvisorRefusal {
    pub completed_phases: Vec<HubGvisorPhaseEvidence>,
    reason: String,
}

impl std::fmt::Display for HubGvisorRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for HubGvisorRefusal {}

fn retain_completed_phases(
    error: anyhow::Error,
    phases: &[HubGvisorPhaseEvidence],
) -> anyhow::Error {
    HubGvisorRefusal {
        completed_phases: phases.to_vec(),
        reason: format!("{error:#}"),
    }
    .into()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationEvidenceV1 {
    schema: String,
    source_commit: String,
    workflow_run_id: u64,
    receipt_sha256: String,
}

impl QualificationEvidenceV1 {
    fn load(path: &Path) -> Result<Self> {
        let evidence: Self = serde_json::from_slice(
            &std::fs::read(path).context("read Hub gVisor qualification manifest")?,
        )
        .context("parse Hub gVisor qualification manifest")?;
        ensure!(
            evidence.schema == QUALIFICATION_SCHEMA_V1
                && lower_hex(&evidence.source_commit, 40)
                && evidence.workflow_run_id > 0
                && lower_hex(&evidence.receipt_sha256, 64),
            "invalid immutable Hub gVisor qualification evidence"
        );
        Ok(evidence)
    }

    fn evidence_id(&self) -> String {
        format!(
            "github:Memorithm/scirust-hub@{}/actions/runs/{}#receipt-sha256:{}",
            self.source_commit, self.workflow_run_id, self.receipt_sha256
        )
    }
}

#[derive(Debug, Serialize)]
struct RequestV1 {
    schema: &'static str,
    task_id: String,
    image: String,
    workspace: PathBuf,
    workspace_quota: GvisorWorkspaceQuotaV1,
    command: Vec<String>,
    environment: Vec<(String, String)>,
    cpu_millis: u64,
    memory_bytes: u64,
    pids_limit: u64,
    timeout_ms: u64,
    max_capture_bytes: usize,
    default_deny_network: bool,
    read_only_root: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptV1 {
    schema: String,
    task_id: String,
    backend_id: String,
    isolation: String,
    isolation_evidence_id: Option<String>,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

pub struct HubGvisorExecutor {
    config: HubGvisorConfig,
}

struct PhaseExecution<'a> {
    task_id: &'a str,
    phase: &'static str,
    workspace: &'a Path,
    workspace_root: &'a Path,
    request_root: &'a Path,
    command: Vec<String>,
    timeout: Duration,
    expected_evidence_id: &'a str,
    source_sha256: &'a str,
    artifact_sha256: Option<String>,
}

impl HubGvisorExecutor {
    pub fn new(config: HubGvisorConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Materialize and execute untrusted Rust only through the qualified Hub adapter.
    pub fn execute_rust_source(&self, source: &str) -> Result<HubGvisorResult> {
        let task_id = format!("papers-{}", Uuid::new_v4().simple());
        self.execute_rust_source_with_task(&task_id, source)
    }

    fn execute_rust_source_with_task(
        &self,
        task_id: &str,
        source: &str,
    ) -> Result<HubGvisorResult> {
        ensure!(
            !source.is_empty() && source.len() <= self.config.max_source_bytes,
            "untrusted Rust source violates the configured byte bound"
        );
        ensure!(
            source.contains("fn main"),
            "isolated Rust candidate must define an explicit main"
        );
        let evidence = QualificationEvidenceV1::load(&self.config.qualification_manifest)?;
        let expected_evidence_id = evidence.evidence_id();
        let workspace_root = self
            .config
            .workspace_root
            .canonicalize()
            .context("canonicalize executor-owned workspace root")?;
        let volume = self.config.workspace_volume.canonicalize()
            .context("canonicalize persistent task volume")?;
        ensure!(volume.parent() == Some(workspace_root.as_path()),
            "persistent task volume must be an immediate child of controller root");
        let control = tempfile::Builder::new()
            .prefix("papers-control-")
            .tempdir_in(&workspace_root)
            .context("materialize private phase inputs outside guest volume")?;
        let workspace = tempfile::Builder::new()
            .prefix("papers-gvisor-")
            .tempdir_in(&volume)
            .context("materialize task below executor-owned workspace root")?;
        std::fs::write(workspace.path().join("main.rs"), source)
            .context("write isolated Rust candidate")?;

        let source_sha256 = format!("{:x}", Sha256::digest(source.as_bytes()));
        let compiled_result = self.execute_phase(PhaseExecution {
            task_id: &format!("{task_id}-compile"),
            phase: "compilation",
            workspace: workspace.path(),
            workspace_root: &workspace_root,
            request_root: control.path(),
            command: vec![
                "rustc".into(),
                "main.rs".into(),
                "-o".into(),
                "candidate".into(),
            ],
            timeout: self.config.compile_timeout,
            expected_evidence_id: &expected_evidence_id,
            source_sha256: &source_sha256,
            artifact_sha256: None,
        });
        let mut compiled = match compiled_result {
            Ok(result) => result,
            Err(error) => return Err(quarantine_refusal(error, workspace, control)),
        };
        if !compiled.success {
            return Ok(compiled);
        }
        // A bounded regular artifact is checked before any candidate execution.
        // This is an admission/postcondition limit, not an OS filesystem quota.
        let artifact = workspace.path().join("candidate");
        let artifact_sha256 = match bounded_artifact_digest(&artifact) {
            Ok(digest) => digest,
            Err(error) => {
                return Err(quarantine_refusal(
                    retain_completed_phases(error, &compiled.phase_evidence),
                    workspace,
                    control,
                ))
            }
        };
        compiled.phase_evidence[0].artifact_sha256 = Some(artifact_sha256.clone());
        let executed_result = self.execute_phase(PhaseExecution {
            task_id: &format!("{task_id}-run"),
            phase: "execution",
            workspace: workspace.path(),
            workspace_root: &workspace_root,
            request_root: control.path(),
            command: vec!["./candidate".into()],
            timeout: self.config.timeout,
            expected_evidence_id: &expected_evidence_id,
            source_sha256: &source_sha256,
            artifact_sha256: Some(artifact_sha256),
        });
        let mut executed = match executed_result {
            Ok(result) => result,
            Err(error) => {
                return Err(quarantine_refusal(
                    retain_completed_phases(error, &compiled.phase_evidence),
                    workspace,
                    control,
                ))
            }
        };
        compiled.phase_evidence.append(&mut executed.phase_evidence);
        executed.phase_evidence = compiled.phase_evidence;
        Ok(executed)
    }

    fn execute_phase(&self, execution: PhaseExecution<'_>) -> Result<HubGvisorResult> {
        let PhaseExecution {
            task_id,
            phase,
            workspace,
            workspace_root,
            request_root,
            command,
            timeout,
            expected_evidence_id,
            source_sha256,
            artifact_sha256,
        } = execution;
        let deadline = Instant::now()
            .checked_add(
                timeout
                    .checked_add(Duration::from_secs(15))
                    .ok_or_else(|| anyhow!("outer Hub duration overflow"))?,
            )
            .ok_or_else(|| anyhow!("outer Hub deadline overflow"))?;
        let request = RequestV1 {
            schema: REQUEST_SCHEMA_V1,
            task_id: task_id.to_owned(),
            image: self.config.image.clone(),
            workspace: workspace.to_path_buf(),
            workspace_quota: self.config.workspace_quota.clone(),
            command,
            environment: vec![
                ("HOME".to_owned(), "/tmp".to_owned()),
                ("RUSTUP_HOME".to_owned(), "/usr/local/rustup".to_owned()),
                ("CARGO_HOME".to_owned(), "/usr/local/cargo".to_owned()),
            ],
            cpu_millis: self.config.cpu_millis,
            memory_bytes: self.config.memory_bytes,
            pids_limit: self.config.pids_limit,
            timeout_ms: u64::try_from(timeout.as_millis())
                .map_err(|_| anyhow!("timeout exceeds Hub request representation"))?,
            max_capture_bytes: self.config.max_capture_bytes,
            default_deny_network: true,
            read_only_root: true,
        };
        let request_path = request_root.join(format!("{phase}-request.json"));
        let request_bytes = serde_json::to_vec(&request)?;
        let request_sha256 = format!("{:x}", Sha256::digest(&request_bytes));
        std::fs::write(&request_path, request_bytes).context("write Hub gVisor request")?;

        let mut command = Command::new(&self.config.executable);
        command
            .arg(&request_path)
            .arg(workspace_root)
            .arg(&self.config.qualification_manifest)
            .arg(&self.config.qualification_receipt);
        let guarded = execute_guarded_command(&mut command, deadline, MAX_CAPTURE_BYTES)
            .map_err(|error| anyhow!("qualified Hub adapter supervision failed: {error}"))?;
        ensure!(
            !guarded.timed_out,
            "SciRust Hub gVisor adapter exceeded its outer deadline"
        );
        let status = guarded.status;
        let stdout = guarded.stdout.bytes;
        let stderr = guarded.stderr.bytes;
        ensure!(
            !guarded.stdout.truncated && !guarded.stderr.truncated,
            "SciRust Hub adapter output exceeded PAPERS bounds"
        );
        let outer_stderr = String::from_utf8_lossy(&stderr);
        let receipt: ReceiptV1 = serde_json::from_slice(&stdout).with_context(|| {
            format!("parse SciRust Hub execution receipt; adapter stderr: {outer_stderr}")
        })?;
        ensure!(
            receipt.schema == RECEIPT_SCHEMA_V1
                && receipt.task_id == task_id
                && receipt.backend_id == "docker-runsc"
                && receipt.isolation == "userspace_kernel"
                && receipt.isolation_evidence_id.as_deref() == Some(expected_evidence_id)
                && !receipt.stdout_truncated
                && !receipt.stderr_truncated,
            "SciRust Hub receipt violates the qualified isolation contract"
        );
        ensure!(
            status.success() == (receipt.exit_code == Some(0) && !receipt.timed_out),
            "Hub process status and execution receipt disagree"
        );
        Ok(HubGvisorResult {
            success: receipt.exit_code == Some(0) && !receipt.timed_out,
            exit_code: receipt.exit_code,
            timed_out: receipt.timed_out,
            stdout: receipt.stdout,
            stderr: receipt.stderr,
            isolation_evidence_id: expected_evidence_id.to_owned(),
            phase_evidence: vec![HubGvisorPhaseEvidence {
                phase,
                task_id: task_id.to_owned(),
                timeout_ms: request.timeout_ms,
                source_sha256: source_sha256.to_owned(),
                artifact_sha256,
                request_sha256,
                receipt_sha256: format!("{:x}", Sha256::digest(&stdout)),
                exit_code: receipt.exit_code,
                timed_out: receipt.timed_out,
                isolation_evidence_id: expected_evidence_id.to_owned(),
            }],
        })
    }
}

fn bounded_artifact_digest(artifact: &Path) -> Result<String> {
    let metadata = std::fs::symlink_metadata(artifact).context("inspect compiled artifact")?;
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_NATIVE_ARTIFACT_BYTES,
        "compiled artifact must be a bounded regular file"
    );
    let mut bytes = Vec::new();
    std::fs::File::open(artifact)?
        .take(MAX_NATIVE_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == metadata.len() && !bytes.is_empty(),
        "compiled artifact changed or is empty"
    );
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn quarantine_refusal(error: anyhow::Error, workspace: tempfile::TempDir, control: tempfile::TempDir) -> anyhow::Error {
    let retained = workspace.keep();
    let inputs = control.keep();
    let reason = format!("{error:#}");
    error.context(format!("{reason}; executor workspace retained for quarantine/reconciliation at {}; private phase inputs retained outside guest at {}; no automatic deletion or replay", retained.display(), inputs.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // ABI fixtures only. These scripts never compile/execute generated source
    // and are not evidence of real sandbox or resource enforcement.
    fn phase_fixture(
        compile_exit: i32,
        compile_timeout: bool,
    ) -> (tempfile::TempDir, HubGvisorConfig) {
        let root = tempfile::tempdir().unwrap();
        let workspace_root = root.path().join("workspaces");
        std::fs::create_dir(&workspace_root).unwrap();
        let workspace_volume = workspace_root.join("volume");
        std::fs::create_dir(&workspace_volume).unwrap();
        let manifest = root.path().join("qualification.json");
        std::fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
                "schema": QUALIFICATION_SCHEMA_V1, "source_commit": "a".repeat(40),
                "workflow_run_id": 17, "receipt_sha256": "b".repeat(64)
            }))
            .unwrap(),
        )
        .unwrap();
        let owner = format!(
            "github:Memorithm/scirust-hub@{}/actions/runs/17#receipt-sha256:{}",
            "a".repeat(40),
            "b".repeat(64)
        );
        let receipt = |phase: &str, exit: i32, timed_out: bool| {
            serde_json::json!({
                "schema": RECEIPT_SCHEMA_V1, "task_id": format!("papers-test-{phase}"),
                "backend_id": "docker-runsc", "isolation": "userspace_kernel",
                "isolation_evidence_id": owner, "exit_code": exit, "timed_out": timed_out,
                "stdout": "PAPERS_GVISOR_OK\n", "stderr": "fixture diagnostic",
                "stdout_truncated": false, "stderr_truncated": false
            })
            .to_string()
        };
        let executable = root.path().join("hub");
        std::fs::write(&executable, format!(
            "#!/bin/sh\ncp \"$1\" '{}'/$(basename \"$1\")\ncase \"$1\" in\n*/compilation-request.json)\n  sandbox=$(jq -r .workspace \"$1\")\n  printf fixture > \"$sandbox/candidate\"\n  printf '%s\\n' '{}'\n  exit {}\n  ;;\n*/execution-request.json)\n  printf called > '{}'/run-called\n  printf '%s\\n' '{}'\n  ;;\n*) exit 3 ;;\nesac\n",
            root.path().display(), receipt("compile", compile_exit, compile_timeout),
            if compile_timeout && compile_exit == 0 { 1 } else { compile_exit }, root.path().display(), receipt("run", 0, false)
        )).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = HubGvisorConfig {
            executable,
            workspace_root,
            workspace_volume,
            workspace_quota: GvisorWorkspaceQuotaV1 { max_bytes: 64 * 1024 * 1024, max_inodes: 256 },
            qualification_manifest: manifest,
            qualification_receipt: root.path().join("receipt.json"),
            image: format!("rust@sha256:{}", "c".repeat(64)),
            cpu_millis: 1000,
            memory_bytes: 512 * 1024 * 1024,
            pids_limit: 64,
            compile_timeout: Duration::from_millis(700),
            timeout: Duration::from_millis(1300),
            max_source_bytes: 4096,
            max_capture_bytes: 65536,
        };
        (root, config)
    }

    #[test]
    fn independent_phase_requests_preserve_receipt_provenance() {
        let (root, config) = phase_fixture(0, false);
        let result = HubGvisorExecutor::new(config)
            .unwrap()
            .execute_rust_source_with_task("papers-test", "fn main() {}")
            .unwrap();
        assert!(result.success);
        assert_eq!(result.phase_evidence.len(), 2);
        assert_eq!(result.phase_evidence[0].timeout_ms, 700);
        assert_eq!(result.phase_evidence[1].timeout_ms, 1300);
        assert_ne!(
            result.phase_evidence[0].task_id,
            result.phase_evidence[1].task_id
        );
        assert_ne!(
            result.phase_evidence[0].receipt_sha256,
            result.phase_evidence[1].receipt_sha256
        );
        for evidence in &result.phase_evidence {
            assert_eq!(evidence.source_sha256.len(), 64);
            assert_eq!(evidence.artifact_sha256.as_ref().unwrap().len(), 64);
            assert_eq!(evidence.request_sha256.len(), 64);
            assert_eq!(evidence.receipt_sha256.len(), 64);
        }
        let compile: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("compilation-request.json")).unwrap(),
        )
        .unwrap();
        let run: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("execution-request.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            compile["command"],
            serde_json::json!(["rustc", "main.rs", "-o", "candidate"])
        );
        assert_eq!(run["command"], serde_json::json!(["./candidate"]));
        for request in [&compile, &run] {
            assert_eq!(request["pids_limit"], 64);
            assert_eq!(request["workspace_quota"]["max_bytes"], 64 * 1024 * 1024);
            assert_eq!(request["workspace_quota"]["max_inodes"], 256);
            let guest = PathBuf::from(request["workspace"].as_str().unwrap());
            assert!(guest.starts_with(root.path().join("workspaces/volume")));
            assert!(!guest.join("compilation-request.json").exists());
            assert!(!guest.join("execution-request.json").exists());
            // The qualified owner defaults guest NPROC to the unchanged wire
            // budget; PAPERS does not silently widen the host task ceiling.
            assert!(request.get("guest_pids_limit").is_none());
        }
    }

    #[test]
    fn quota_bounds_fail_before_materialization_or_dispatch() {
        let (root, config) = phase_fixture(0, false);
        for invalid in [0, 1024 * 1024 * 1024 + 1, u64::MAX] {
            let mut bad = config.clone();
            bad.workspace_quota.max_bytes = invalid;
            assert!(HubGvisorExecutor::new(bad).is_err());
        }
        for invalid in [0, 65537, u64::MAX] {
            let mut bad = config.clone();
            bad.workspace_quota.max_inodes = invalid;
            assert!(HubGvisorExecutor::new(bad).is_err());
        }
        assert_eq!(std::fs::read_dir(&config.workspace_volume).unwrap().count(), 0);
        assert!(!root.path().join("run-called").exists());
    }

    #[test]
    fn pid_budget_matches_owner_before_workspace_or_dispatch() {
        let (root, config) = phase_fixture(0, false);
        for invalid in [0, 4097, u64::MAX] {
            let mut bad = config.clone();
            bad.pids_limit = invalid;
            assert!(HubGvisorExecutor::new(bad).is_err());
        }
        for valid in [1, 4096] {
            let mut accepted = config.clone();
            accepted.pids_limit = valid;
            assert!(HubGvisorExecutor::new(accepted).is_ok());
        }
        assert_eq!(std::fs::read_dir(&config.workspace_volume).unwrap().count(), 0);
        assert!(!root.path().join("run-called").exists());
    }

    #[test]
    fn failed_or_timed_out_compilation_never_dispatches_execution() {
        for (exit, timed_out) in [(1, false), (0, true)] {
            let (root, config) = phase_fixture(exit, timed_out);
            let result = HubGvisorExecutor::new(config)
                .unwrap()
                .execute_rust_source_with_task("papers-test", "fn main() {}")
                .unwrap();
            assert!(!result.success);
            assert_eq!(result.phase_evidence.len(), 1);
            assert_eq!(result.phase_evidence[0].phase, "compilation");
            assert!(!root.path().join("run-called").exists());
        }
    }

    #[test]
    fn phase_deadline_overflow_and_submillisecond_budget_fail_before_spawn() {
        let (_root, config) = phase_fixture(0, false);
        for duration in [
            Duration::MAX,
            Duration::from_nanos(1),
            Duration::ZERO,
            Duration::from_millis(3_600_001),
        ] {
            let mut bad = config.clone();
            bad.compile_timeout = duration;
            assert!(HubGvisorExecutor::new(bad).is_err());
        }
    }

    #[test]
    fn wrong_phase_identity_never_dispatches_execution() {
        let (root, config) = phase_fixture(0, false);
        let script = std::fs::read_to_string(&config.executable)
            .unwrap()
            .replace("papers-test-compile", "wrong-task-compile");
        std::fs::write(&config.executable, script).unwrap();
        assert!(HubGvisorExecutor::new(config)
            .unwrap()
            .execute_rust_source_with_task("papers-test", "fn main() {}")
            .is_err());
        assert!(!root.path().join("run-called").exists());
    }

    #[test]
    fn execution_refusal_retains_verified_compile_evidence() {
        let (_root, config) = phase_fixture(0, false);
        let script = std::fs::read_to_string(&config.executable)
            .unwrap()
            .replace("papers-test-run", "wrong-task-run");
        std::fs::write(&config.executable, script).unwrap();
        let error = HubGvisorExecutor::new(config)
            .unwrap()
            .execute_rust_source_with_task("papers-test", "fn main() {}")
            .unwrap_err();
        let phases = &error
            .downcast_ref::<HubGvisorRefusal>()
            .unwrap()
            .completed_phases;
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].phase, "compilation");
        assert_eq!(phases[0].timeout_ms, 700);
        assert_eq!(phases[0].receipt_sha256.len(), 64);
        assert_eq!(phases[0].artifact_sha256.as_ref().unwrap().len(), 64);
        assert!(error.to_string().contains("qualified isolation contract"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detached_adapter_pipe_refusal_preserves_workspace() {
        let (root, config) = phase_fixture(0, false);
        std::fs::write(&config.executable,
            "#!/bin/sh\nGUARD_READY=\"$(dirname \"$1\")/ready\"; export GUARD_READY\nsetsid sh -c 'printf ready > \"$GUARD_READY\"; exec sleep 3' &\nwhile [ ! -e \"$GUARD_READY\" ]; do sleep 0.01; done\nprintf '{}'\nexit 0\n"
        ).unwrap();
        let started = Instant::now();
        let error = HubGvisorExecutor::new(config)
            .unwrap()
            .execute_rust_source_with_task("papers-test", "fn main() {}")
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(error.to_string().contains("capture incomplete"));
        assert!(error.to_string().contains("quarantine/reconciliation"));
        let retained = std::fs::read_dir(root.path().join("workspaces/volume"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(retained.len(), 1);
        assert!(retained[0].join("main.rs").is_file());
        assert!(!retained[0].join("compilation-request.json").exists());
        assert!(std::fs::read_dir(root.path().join("workspaces")).unwrap()
            .any(|entry| entry.unwrap().path().join("compilation-request.json").is_file()));
    }

    #[test]
    fn native_artifact_refuses_empty_oversized_and_symlink_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("candidate");
        let file = std::fs::File::create(&path).unwrap();
        assert!(bounded_artifact_digest(&path).is_err());
        file.set_len(MAX_NATIVE_ARTIFACT_BYTES + 1).unwrap();
        assert!(bounded_artifact_digest(&path).is_err());
        let link = root.path().join("linked");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(bounded_artifact_digest(&link).is_err());
    }

    #[test]
    fn rejects_mutable_image_before_execution() {
        let root = tempfile::tempdir().unwrap();
        let error = HubGvisorExecutor::new(HubGvisorConfig {
            executable: root.path().join("hub"),
            workspace_root: root.path().to_path_buf(),
            workspace_volume: root.path().join("volume"),
            workspace_quota: GvisorWorkspaceQuotaV1 { max_bytes: 64 * 1024 * 1024, max_inodes: 256 },
            qualification_manifest: root.path().join("qualification.json"),
            qualification_receipt: root.path().join("receipt.json"),
            image: "rust:latest".to_owned(),
            cpu_millis: 1000,
            memory_bytes: 512 * 1024 * 1024,
            pids_limit: 64,
            compile_timeout: Duration::from_secs(60),
            timeout: Duration::from_secs(2),
            max_source_bytes: 4096,
            max_capture_bytes: 65536,
        })
        .err()
        .expect("mutable image must fail");
        assert!(error.to_string().contains("immutable"));
    }
}
