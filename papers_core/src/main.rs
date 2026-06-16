use papers_core::cli::{parse, Commands};
use papers_core::database::Database;
use papers_core::evolution::EvolutionLoop;
use papers_core::models::{CognitionItem, EvolutionConfig};
use papers_core::cognition::CognitionStore;

fn main() {
    env_logger::init();
    let cli = parse();

    match cli.command {
        Commands::Evolve {
            task,
            rounds,
            policy,
            candidates,
            model,
            llm,
            output,
        } => {
            println!("PAPERS V2 - Rust Evolution Engine");
            println!("Task: {}", task);
            println!("Rounds: {}, Policy: {}, Candidates: {}", rounds, policy, candidates);
            println!("Model: {}, LLM: {}", model, llm);

            let config = EvolutionConfig {
                task_description: task.clone(),
                max_rounds: rounds,
                sampling_policy: policy.clone(),
                n_candidates_per_round: candidates,
                ..Default::default()
            };

            let mut evo = EvolutionLoop::new(config);
            let result = evo.run(|_query| {
                // Dummy evaluator
                (true, rand::random::<f64>() * 0.5 + 0.3)
            });

            println!("Result: best={:.4}, candidates={}, time={:.1}s",
                result.best_score, result.total_candidates, result.total_time_secs);
            println!("Output: {}", output);
        }

        Commands::Status { db } => {
            let db = Database::new("greedy");
            println!("Database: {} nodes", db.len());
        }

        Commands::Seed { file } => {
            println!("Seeding cognition from: {}", file);
            let mut store = CognitionStore::new();
            store.add(CognitionItem::new(
                "Cognition seed".into(),
                file.clone(),
                vec!["seed".into()],
            ));
            println!("Cognition store: {} items", store.len());
        }
    }
}
