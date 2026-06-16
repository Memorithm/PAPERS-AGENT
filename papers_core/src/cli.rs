use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "papers", about = "PAPERS V2 - Rust evolution engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run evolution loop
    Evolve {
        /// Task description
        #[arg(short, long)]
        task: String,
        /// Max rounds
        #[arg(short, long, default_value = "50")]
        rounds: usize,
        /// Sampling policy
        #[arg(long, default_value = "greedy")]
        policy: String,
        /// Candidates per round
        #[arg(short, long, default_value = "3")]
        candidates: usize,
        /// LLM model
        #[arg(long, default_value = "gemma4:e2b")]
        model: String,
        /// Use LLM (set to false for benchmark)
        #[arg(long, default_value = "true")]
        llm: bool,
        /// Output directory
        #[arg(short, long, default_value = "./output")]
        output: String,
    },

    /// Show database stats
    Status {
        #[arg(long, default_value = "./evolution_db.json")]
        db: String,
    },

    /// Seed cognition from JSON file
    Seed {
        #[arg(short, long)]
        file: String,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}
