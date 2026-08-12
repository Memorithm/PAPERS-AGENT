use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use papers_core::scientific_contract::{
    sha256_hex, ExperimentProposal, ResourceLimits, ScientificBundle,
    EXPERIMENT_PROPOSAL_SCHEMA,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "papers-experiment",
    version,
    about = "Build an explicit experiment-proposal-v1 from a validated scientific bundle"
)]
struct Args {
    /// Scientific bundle produced by `papers-contract`.
    #[arg(long)]
    bundle: PathBuf,

    /// Claim id to link. Repeat --claim for multiple claims.
    #[arg(long = "claim", required = true)]
    claim_ids: Vec<String>,

    /// Falsifiable hypothesis to test. This is operator/agent supplied, never inferred here.
    #[arg(long)]
    hypothesis: String,

    /// Component/workspace target under test.
    #[arg(long)]
    target: String,

    /// Intervention to evaluate. It remains data; this command does not execute it.
    #[arg(long)]
    intervention: String,

    /// Frozen baseline used for comparison.
    #[arg(long)]
    baseline: String,

    /// Metric to measure. Repeat --metric; at least one is required.
    #[arg(long = "metric", required = true)]
    metrics: Vec<String>,

    /// Explicit acceptance criterion. Repeat --accept; at least one is required.
    #[arg(long = "accept", required = true)]
    acceptance_criteria: Vec<String>,

    /// Explicit rejection criterion. Repeat --reject as needed.
    #[arg(long = "reject")]
    rejection_criteria: Vec<String>,

    /// Safety constraint. Repeat --safety as needed.
    #[arg(long = "safety")]
    safety_constraints: Vec<String>,

    #[arg(long)]
    expected_direction: Option<String>,

    #[arg(long)]
    expected_effect: Option<String>,

    #[arg(long)]
    workload: Option<String>,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    #[arg(long, default_value_t = 5)]
    repetitions: u32,

    #[arg(long)]
    timeout_seconds: Option<u64>,

    #[arg(long)]
    max_memory_bytes: Option<u64>,

    #[arg(long)]
    max_output_bytes: Option<u64>,

    /// Output JSON. Defaults to experiment_proposal.json next to the bundle.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print JSON instead of writing a file.
    #[arg(long, default_value_t = false)]
    stdout: bool,
}

#[derive(Serialize)]
struct ExperimentIdentity<'a> {
    paper_id: &'a str,
    claim_ids: &'a [String],
    hypothesis: &'a str,
    target: &'a str,
    intervention: &'a str,
    baseline: &'a str,
    metrics: &'a [String],
    acceptance_criteria: &'a [String],
    rejection_criteria: &'a [String],
    safety_constraints: &'a [String],
    expected_direction: &'a Option<String>,
    expected_effect: &'a Option<String>,
    workload: &'a Option<String>,
    seed: u64,
    repetitions: u32,
    timeout_seconds: Option<u64>,
    max_memory_bytes: Option<u64>,
    max_output_bytes: Option<u64>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    validate_cli(&args)?;

    let raw = fs::read_to_string(&args.bundle)
        .with_context(|| format!("cannot read {}", args.bundle.display()))?;
    let bundle: ScientificBundle = serde_json::from_str(&raw)
        .with_context(|| format!("invalid ScientificBundle JSON in {}", args.bundle.display()))?;
    bundle.validate().map_err(anyhow::Error::msg)?;

    let known_claims: BTreeSet<&str> = bundle.claims.iter().map(|claim| claim.id.as_str()).collect();
    for claim_id in &args.claim_ids {
        if !known_claims.contains(claim_id.as_str()) {
            anyhow::bail!("unknown claim id in bundle: {claim_id}");
        }
    }

    let identity = ExperimentIdentity {
        paper_id: &bundle.paper.id,
        claim_ids: &args.claim_ids,
        hypothesis: &args.hypothesis,
        target: &args.target,
        intervention: &args.intervention,
        baseline: &args.baseline,
        metrics: &args.metrics,
        acceptance_criteria: &args.acceptance_criteria,
        rejection_criteria: &args.rejection_criteria,
        safety_constraints: &args.safety_constraints,
        expected_direction: &args.expected_direction,
        expected_effect: &args.expected_effect,
        workload: &args.workload,
        seed: args.seed,
        repetitions: args.repetitions,
        timeout_seconds: args.timeout_seconds,
        max_memory_bytes: args.max_memory_bytes,
        max_output_bytes: args.max_output_bytes,
    };
    let id_payload =
        serde_json::to_vec(&identity).context("cannot serialize experiment identity payload")?;
    let digest = sha256_hex(&id_payload);

    let proposal = ExperimentProposal {
        schema: EXPERIMENT_PROPOSAL_SCHEMA.into(),
        id: format!("exp-{}", &digest[..24]),
        hypothesis: args.hypothesis,
        claim_ids: args.claim_ids,
        target_component: args.target,
        intervention: args.intervention,
        baseline: args.baseline,
        metrics: args.metrics,
        expected_direction: args.expected_direction,
        expected_effect: args.expected_effect,
        workload: args.workload,
        seed: args.seed,
        repetitions: args.repetitions,
        acceptance_criteria: args.acceptance_criteria,
        rejection_criteria: args.rejection_criteria,
        resource_limits: ResourceLimits {
            timeout_seconds: args.timeout_seconds,
            max_memory_bytes: args.max_memory_bytes,
            max_output_bytes: args.max_output_bytes,
        },
        safety_constraints: args.safety_constraints,
        provenance: bundle.provenance,
    };
    proposal.validate().map_err(anyhow::Error::msg)?;

    let json =
        serde_json::to_string_pretty(&proposal).context("cannot serialize experiment proposal")?;
    if args.stdout {
        println!("{json}");
        return Ok(());
    }

    let output = args.output.unwrap_or_else(|| {
        args.bundle
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("experiment_proposal.json")
    });
    fs::write(&output, json)
        .with_context(|| format!("cannot write {}", output.display()))?;
    println!("{}", output.display());
    Ok(())
}

fn validate_cli(args: &Args) -> Result<()> {
    for (name, value) in [
        ("hypothesis", args.hypothesis.as_str()),
        ("target", args.target.as_str()),
        ("intervention", args.intervention.as_str()),
        ("baseline", args.baseline.as_str()),
    ] {
        if value.trim().is_empty() {
            anyhow::bail!("--{name} must be non-empty");
        }
    }
    if args.metrics.iter().any(|metric| metric.trim().is_empty()) {
        anyhow::bail!("--metric values must be non-empty");
    }
    if args
        .acceptance_criteria
        .iter()
        .any(|criterion| criterion.trim().is_empty())
    {
        anyhow::bail!("--accept values must be non-empty");
    }
    if args.repetitions == 0 {
        anyhow::bail!("--repetitions must be >= 1");
    }
    for (name, value) in [
        ("timeout-seconds", args.timeout_seconds),
        ("max-memory-bytes", args.max_memory_bytes),
        ("max-output-bytes", args.max_output_bytes),
    ] {
        if value == Some(0) {
            anyhow::bail!("--{name} must be > 0 when supplied");
        }
    }
    Ok(())
}
