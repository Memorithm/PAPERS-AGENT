//! Read-only reconciliation of an existing lab experiment; never resubmits.
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use papers_core::ccos::LabClient;

#[derive(Debug, Parser)]
#[command(
    name = "papers-lab",
    version,
    about = "Read-only Research Lab reconciliation"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read the state of an already identified experiment without any POST.
    Reconcile {
        #[arg(long)]
        lab_url: String,
        #[arg(long)]
        experiment_id: String,
    },
}

fn main() -> Result<()> {
    let Command::Reconcile {
        lab_url,
        experiment_id,
    } = Args::parse().command;
    anyhow::ensure!(
        !experiment_id.trim().is_empty(),
        "experiment ID must not be empty"
    );
    let mut client = LabClient::new(lab_url);
    match std::env::var("PAPERS_LAB_API_KEY") {
        Ok(key) => {
            client = client.with_api_key(key);
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(e) => return Err(e).context("invalid PAPERS_LAB_API_KEY"),
    }
    let status = client
        .experiment_status(&experiment_id)
        .map_err(anyhow::Error::msg)?;
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconciliation_requires_lab_url_and_id() {
        assert!(Args::try_parse_from(["papers-lab", "reconcile"]).is_err());
        assert!(Args::try_parse_from(["papers-lab", "submit"]).is_err());
        assert!(Args::try_parse_from([
            "papers-lab",
            "reconcile",
            "--lab-url",
            "http://127.0.0.1:1",
            "--experiment-id",
            "exp-1"
        ])
        .is_ok());
    }
}
