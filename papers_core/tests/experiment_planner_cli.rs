use std::fs;
use std::process::Command;

use papers_core::scientific_contract::{ExperimentProposal, EXPERIMENT_PROPOSAL_SCHEMA};
use tempfile::tempdir;

const FIXTURE: &str = include_str!("fixtures/scientific_bundle_v1.json");

fn planner(bundle: &std::path::Path, claim: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_papers-experiment"))
        .args([
            "--bundle",
            bundle.to_str().unwrap(),
            "--claim",
            claim,
            "--hypothesis",
            "TiledMethod lowers median latency without changing outputs",
            "--target",
            "src/kernel.rs",
            "--intervention",
            "candidate implementation using TiledMethod",
            "--baseline",
            "frozen main implementation",
            "--metric",
            "latency_ns",
            "--accept",
            "median latency improves by at least 5% and tests pass",
            "--safety",
            "offline",
            "--repetitions",
            "20",
            "--timeout-seconds",
            "60",
            "--max-output-bytes",
            "8388608",
            "--stdout",
        ])
        .output()
        .expect("run papers-experiment")
}

#[test]
fn planner_emits_valid_deterministic_proposal() {
    let dir = tempdir().unwrap();
    let bundle = dir.path().join("bundle.json");
    fs::write(&bundle, FIXTURE).unwrap();

    let first = planner(&bundle, "method-fixture-1");
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let proposal: ExperimentProposal = serde_json::from_slice(&first.stdout).unwrap();
    proposal.validate().unwrap();
    assert_eq!(proposal.schema, EXPERIMENT_PROPOSAL_SCHEMA);
    assert_eq!(proposal.claim_ids, vec!["method-fixture-1"]);
    assert_eq!(proposal.metrics, vec!["latency_ns"]);
    assert_eq!(proposal.repetitions, 20);
    assert_eq!(proposal.resource_limits.timeout_seconds, Some(60));
    assert_eq!(proposal.resource_limits.max_output_bytes, Some(8_388_608));

    let second = planner(&bundle, "method-fixture-1");
    assert!(second.status.success());
    let replay: ExperimentProposal = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(proposal.id, replay.id);
}

#[test]
fn planner_rejects_claim_not_present_in_bundle() {
    let dir = tempdir().unwrap();
    let bundle = dir.path().join("bundle.json");
    fs::write(&bundle, FIXTURE).unwrap();

    let output = planner(&bundle, "missing-claim");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown claim id"));
}
