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

    let json =
        serde_json::to_string_pretty(&bundle).context("cannot serialize scientific bundle")?;

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
        path.set_file_name(format!("{stem}.scientific_bundle.json"));
        path
    });

    fs::write(&output, json).with_context(|| format!("cannot write {}", output.display()))?;

    println!("{}", output.display());
    Ok(())
}
