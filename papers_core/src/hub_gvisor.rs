//! Typed PAPERS consumer for the qualified SciRust Hub gVisor boundary.
//! Qualification is rechecked by the exact Hub binary before every execution.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const REQUEST_SCHEMA_V1: &str = "scirust-hub.gvisor-execution-request/v1";
const RECEIPT_SCHEMA_V1: &str = "scirust-hub.gvisor-execution-receipt/v1";
const QUALIFICATION_SCHEMA_V1: &str = "scirust-hub.gvisor-qualification-evidence/v1";
const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct HubGvisorConfig {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
    pub qualification_manifest: PathBuf,
    pub qualification_receipt: PathBuf,
    pub image: String,
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub pids_limit: u64,
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
        ensure!(self.memory_bytes >= 16 * 1024 * 1024, "memory bound is too small");
        ensure!(self.pids_limit > 0, "pids_limit must be non-zero");
        ensure!(!self.timeout.is_zero(), "timeout must be non-zero");
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
        let workspace = tempfile::Builder::new()
            .prefix("papers-gvisor-")
            .tempdir_in(&workspace_root)
            .context("materialize task below executor-owned workspace root")?;
        std::fs::write(workspace.path().join("main.rs"), source)
            .context("write isolated Rust candidate")?;

        let request = RequestV1 {
            schema: REQUEST_SCHEMA_V1,
            task_id: task_id.to_owned(),
            image: self.config.image.clone(),
            workspace: workspace.path().to_path_buf(),
            command: vec![
                "bash".to_owned(),
                "-c".to_owned(),
                "rustc main.rs -o candidate && ./candidate".to_owned(),
            ],
            environment: vec![
                ("HOME".to_owned(), "/tmp".to_owned()),
                ("RUSTUP_HOME".to_owned(), "/usr/local/rustup".to_owned()),
                ("CARGO_HOME".to_owned(), "/usr/local/cargo".to_owned()),
            ],
            cpu_millis: self.config.cpu_millis,
            memory_bytes: self.config.memory_bytes,
            pids_limit: self.config.pids_limit,
            timeout_ms: u64::try_from(self.config.timeout.as_millis())
                .map_err(|_| anyhow!("timeout exceeds Hub request representation"))?,
            max_capture_bytes: self.config.max_capture_bytes,
            default_deny_network: true,
            read_only_root: true,
        };
        let request_path = workspace.path().join("request.json");
        std::fs::write(&request_path, serde_json::to_vec(&request)?)
            .context("write Hub gVisor request")?;

        let mut child = Command::new(&self.config.executable)
            .arg(&request_path)
            .arg(&workspace_root)
            .arg(&self.config.qualification_manifest)
            .arg(&self.config.qualification_receipt)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("start qualified SciRust Hub gVisor adapter")?;
        let stdout = spawn_reader(child.stdout.take(), MAX_CAPTURE_BYTES);
        let stderr = spawn_reader(child.stderr.take(), MAX_CAPTURE_BYTES);
        let deadline = Instant::now()
            .checked_add(self.config.timeout + Duration::from_secs(15))
            .ok_or_else(|| anyhow!("outer Hub deadline overflow"))?;
        let status = loop {
            match child.try_wait()? {
                Some(status) => break status,
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(anyhow!("SciRust Hub gVisor adapter exceeded its outer deadline"));
                }
            }
        };
        let (stdout, stdout_truncated) = join_reader(stdout)?;
        let (stderr, stderr_truncated) = join_reader(stderr)?;
        ensure!(
            !stdout_truncated && !stderr_truncated,
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
                && receipt.isolation_evidence_id.as_deref() == Some(&expected_evidence_id)
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
            isolation_evidence_id: expected_evidence_id,
        })
    }
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn spawn_reader<R: Read + Send + 'static>(
    pipe: Option<R>,
    limit: usize,
) -> JoinHandle<std::io::Result<(Vec<u8>, bool)>> {
    thread::spawn(move || {
        let mut pipe =
            pipe.ok_or_else(|| std::io::Error::other("Hub capture pipe unavailable"))?;
        let mut captured = Vec::with_capacity(limit.min(8192));
        let mut buffer = [0_u8; 8192];
        let mut truncated = false;
        loop {
            let count = pipe.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            let remaining = limit.saturating_sub(captured.len());
            let kept = remaining.min(count);
            captured.extend_from_slice(&buffer[..kept]);
            truncated |= kept < count;
        }
        Ok((captured, truncated))
    })
}

fn join_reader(
    reader: JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
) -> Result<(Vec<u8>, bool)> {
    reader
        .join()
        .map_err(|_| anyhow!("Hub capture reader panicked"))?
        .context("read Hub adapter output")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn consumes_only_exact_qualified_receipt() {
        let root = tempfile::tempdir().unwrap();
        let workspace_root = root.path().join("workspaces");
        std::fs::create_dir(&workspace_root).unwrap();
        let manifest = root.path().join("qualification.json");
        std::fs::write(
            &manifest,
            format!(
                "{{\"schema\":\"{}\",\"source_commit\":\"{}\",\"workflow_run_id\":17,\"receipt_sha256\":\"{}\"}}",
                QUALIFICATION_SCHEMA_V1,
                "a".repeat(40),
                "b".repeat(64)
            ),
        )
        .unwrap();
        let evidence_id = format!(
            "github:Memorithm/scirust-hub@{}/actions/runs/17#receipt-sha256:{}",
            "a".repeat(40),
            "b".repeat(64)
        );
        let executable = root.path().join("hub");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{}'\n",
                serde_json::json!({
                    "schema": RECEIPT_SCHEMA_V1,
                    "task_id": "papers-test",
                    "backend_id": "docker-runsc",
                    "isolation": "userspace_kernel",
                    "isolation_evidence_id": evidence_id,
                    "exit_code": 0,
                    "timed_out": false,
                    "stdout": "PAPERS_GVISOR_OK\n",
                    "stderr": "",
                    "stdout_truncated": false,
                    "stderr_truncated": false
                })
            ),
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let executor = HubGvisorExecutor::new(HubGvisorConfig {
            executable,
            workspace_root,
            qualification_manifest: manifest,
            qualification_receipt: root.path().join("receipt.json"),
            image: format!("rust@sha256:{}", "c".repeat(64)),
            cpu_millis: 1000,
            memory_bytes: 512 * 1024 * 1024,
            pids_limit: 64,
            timeout: Duration::from_secs(2),
            max_source_bytes: 4096,
            max_capture_bytes: 65536,
        })
        .unwrap();
        let result = executor
            .execute_rust_source_with_task(
                "papers-test",
                "fn main() { println!(\"PAPERS_GVISOR_OK\"); }",
            )
            .unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("PAPERS_GVISOR_OK"));
        assert!(result.isolation_evidence_id.contains("actions/runs/17"));
    }

    #[test]
    fn rejects_mutable_image_before_execution() {
        let root = tempfile::tempdir().unwrap();
        let error = HubGvisorExecutor::new(HubGvisorConfig {
            executable: root.path().join("hub"),
            workspace_root: root.path().to_path_buf(),
            qualification_manifest: root.path().join("qualification.json"),
            qualification_receipt: root.path().join("receipt.json"),
            image: "rust:latest".to_owned(),
            cpu_millis: 1000,
            memory_bytes: 512 * 1024 * 1024,
            pids_limit: 64,
            timeout: Duration::from_secs(2),
            max_source_bytes: 4096,
            max_capture_bytes: 65536,
        })
        .err()
        .expect("mutable image must fail");
        assert!(error.to_string().contains("immutable"));
    }
}
