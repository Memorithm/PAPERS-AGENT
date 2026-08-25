use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use papers_core::engine::AnalysisReport;
use papers_core::scientific_contract::{ModelProvenance, ScientificBundle};

#[derive(Debug, Parser)]
#[command(
    name = "papers-contract",
    version,
    about = "Export PAPERS analysis.json as a versioned Memorithm scientific bundle"
)]
struct Args {
    /// PAPERS analysis.json produced by `papers analyze`.
    #[arg(short, long)]
    input: PathBuf,

    /// Output path. Defaults to <input>.scientific_bundle.json.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Provider used for model-backed analysis (e.g. ollama, openai-compatible).
    /// Must be supplied together with --model.
    #[arg(long)]
    provider: Option<String>,

    /// Exact model identity used for analysis. Must be supplied with --provider.
    #[arg(long)]
    model: Option<String>,

    /// Optional lowercase SHA-256 of the prompt/model/config envelope.
    #[arg(long)]
    model_config_sha256: Option<String>,

    /// Validate and print the bundle without writing a file.
    #[arg(long, default_value_t = false)]
    stdout: bool,

    /// Output format: json (full bundle), jsonl (streaming records),
    /// csv (flat claims table).
    #[arg(long, default_value = "json")]
    format: String,

    /// CCOS Research Lab base URL: submit the validated bundle after export
    /// (e.g. http://127.0.0.1:8080).
    #[arg(long)]
    lab_url: Option<String>,

    /// Optional bearer token for the Research Lab API.
    #[arg(long)]
    lab_key: Option<String>,

    /// If > 0, poll the lab until the experiment reaches a terminal state or
    /// the delay elapses.
    #[arg(long, default_value_t = 0)]
    wait_secs: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let raw = fs::read_to_string(&args.input)
        .with_context(|| format!("cannot read {}", args.input.display()))?;
    let report: AnalysisReport = serde_json::from_str(&raw)
        .with_context(|| format!("invalid AnalysisReport JSON in {}", args.input.display()))?;

    let model = match (args.provider, args.model) {
        (Some(provider), Some(model)) => Some(ModelProvenance {
            provider,
            model,
            config_sha256: args.model_config_sha256,
        }),
        (None, None) => {
            if args.model_config_sha256.is_some() {
                anyhow::bail!("--model-config-sha256 requires --provider and --model");
            }
            None
        }
        _ => anyhow::bail!("--provider and --model must be supplied together"),
    };

    let bundle =
        ScientificBundle::from_analysis_report(&report, model).map_err(anyhow::Error::msg)?;
    bundle.validate().map_err(anyhow::Error::msg)?;

    let json = match args.format.as_str() {
        "json" => {
            serde_json::to_string_pretty(&bundle).context("cannot serialize scientific bundle")?
        }
        "jsonl" => bundle.to_jsonl().map_err(anyhow::Error::msg)?,
        "csv" => bundle.claims_to_csv(),
        other => anyhow::bail!("format inconnu '{other}' (attendu: json, jsonl, csv)"),
    };

    let extension = match args.format.as_str() {
        "jsonl" => "jsonl",
        "csv" => "csv",
        _ => "json",
    };

    if args.stdout {
        println!("{json}");
        return Ok(());
    }

    let output = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("analysis")
            .to_string();
        path.set_file_name(format!("{stem}.scientific_bundle.{extension}"));
        path
    });

    fs::write(&output, json.clone())
        .with_context(|| format!("cannot write {}", output.display()))?;

    println!("{}", output.display());

    if let Some(ref lab_url) = args.lab_url {
        submit_to_lab(lab_url, args.lab_key.as_deref(), args.wait_secs, &json)?;
    }

    Ok(())
}

/// Soumet le bundle au Research Lab et affiche l'accusé / statut final.
fn submit_to_lab(
    lab_url: &str,
    api_key: Option<&str>,
    wait_secs: u64,
    bundle_json: &str,
) -> Result<()> {
    use papers_core::ccos::LabClient;

    let bundle: serde_json::Value =
        serde_json::from_str(bundle_json).context("bundle re-serialization failed")?;

    let mut client = LabClient::new(lab_url);
    if let Some(key) = api_key {
        client = client.with_api_key(key);
    }

    let submission = client
        .submit_experiment(&bundle)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("cannot submit bundle to {lab_url}"))?;
    println!(
        "🧪 Submitted experiment {} (state: {:?})",
        submission.id, submission.state
    );

    if wait_secs > 0 {
        let status = client
            .wait_for_completion(
                &submission.id,
                std::time::Duration::from_secs(wait_secs),
                std::time::Duration::from_millis(500),
            )
            .map_err(anyhow::Error::msg)?;
        println!("📊 Experiment {}: {:?}", status.id, status.state);
        if !status.state.is_success() {
            anyhow::bail!("experiment did not complete successfully");
        }
    }

    Ok(())
}
