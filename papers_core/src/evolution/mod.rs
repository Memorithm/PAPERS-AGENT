pub mod researcher;
pub mod engineer;
pub mod analyzer;

use std::time::Instant;

use crate::cognition::CognitionStore;
use crate::database::Database;
use crate::llm::LlmClient;
use crate::models::*;

pub use self::analyzer::{AnalysisOutput, Analyzer};
pub use self::engineer::{EngineerOutput, Engineer};
pub use self::researcher::{ResearcherOutput, Researcher};

pub struct EvolutionLoop {
    pub config: EvolutionConfig,
    pub database: Database,
    pub cognition: CognitionStore,
    pub researcher: Researcher,
    pub engineer: Engineer,
}

impl EvolutionLoop {
    pub fn new(config: EvolutionConfig) -> Self {
        let sampler_name = config.sampling_policy.clone();
        let task = config.task_description.clone();
        Self {
            config,
            database: Database::new(&sampler_name),
            cognition: CognitionStore::new(),
            researcher: Researcher::new(&task),
            engineer: Engineer::new(3600),
        }
    }

    pub fn seed_cognition(&mut self, items: Vec<CognitionItem>) {
        self.cognition.add_batch(items);
    }

    /// Runs the full Researcher → Engineer → Analyzer evolution pipeline.
    pub fn run_advanced(
        &mut self,
        llm: &LlmClient,
        task_description: &str,
    ) -> EvolutionResult {
        let start = Instant::now();
        let mut best_score = 0.0_f64;
        let mut best_node: Option<Node> = None;
        let mut rounds_without_improvement = 0;
        let mut total_candidates = 0;
        let mut stopped_early = false;

        for round in 1..=self.config.max_rounds {
            let context_nodes = self.database.sample(self.config.n_context_nodes);
            let context_query = context_nodes
                .first()
                .map(|n| n.analysis.clone())
                .unwrap_or_default();
            let cognition_items = self.cognition.retrieve(&context_query, self.config.n_cognition);

            for i in 0..self.config.n_candidates_per_round {
                let parent = if i > 0 { self.database.best() } else { None };
                let parent_program = parent.map(|n| n.code.as_str()).unwrap_or("");

                let output = self.researcher.generate(
                    llm,
                    task_description,
                    &context_nodes,
                    &cognition_items,
                    parent_program,
                    parent.is_some(),
                );

                let result = self.engineer.execute(&output.program, None::<fn(&str) -> EngineerOutput>);
                let score = Engineer::compute_fitness(&result.metrics, result.score, None);

                total_candidates += 1;

                let mut node = Node::new(
                    output.motivation.clone(),
                    output.program.clone(),
                    if result.success { score } else { 0.0 },
                );
                node.results.insert("success".into(), serde_json::Value::Bool(result.success));
                node.results.insert("score".into(), serde_json::Value::Number(serde_json::Number::from_f64(score).unwrap_or(serde_json::Number::from(0))));
                node.results.insert("runtime_secs".into(), serde_json::json!(result.runtime_secs));
                if let Some(ref err) = result.error {
                    node.results.insert("error".into(), serde_json::json!(err));
                }
                node.analysis = format!("score={:.4}, success={}", score, result.success);
                node.score = score;

                self.database.add(node.clone());

                if score > best_score {
                    best_score = score;
                    best_node = Some(node);
                    rounds_without_improvement = 0;
                } else {
                    rounds_without_improvement += 1;
                }
            }

            if let Some(target) = self.config.target_score {
                if best_score >= target {
                    stopped_early = true;
                    break;
                }
            }

            if rounds_without_improvement >= self.config.patience {
                stopped_early = true;
                break;
            }

            if round % 10 == 0 {
                self.database.prune_bottom(50);
            }
        }

        EvolutionResult {
            success: total_candidates > 0,
            best_score,
            best_node,
            total_rounds: if stopped_early {
                self.config.max_rounds.min(
                    (total_candidates / self.config.n_candidates_per_round.max(1)) + 1,
                )
            } else {
                self.config.max_rounds
            },
            total_candidates,
            total_time_secs: start.elapsed().as_secs_f64(),
            stopped_early,
        }
    }

    pub fn run<F>(&mut self, mut evaluate: F) -> EvolutionResult
    where
        F: FnMut(&str) -> (bool, f64),
    {
        let start = Instant::now();
        let mut best_score = 0.0_f64;
        let mut best_node: Option<Node> = None;
        let mut rounds_without_improvement = 0;
        let mut total_candidates = 0;
        let mut stopped_early = false;

        for round in 1..=self.config.max_rounds {
            let context_nodes = self.database.sample(self.config.n_context_nodes);
            let context_query = context_nodes
                .first()
                .map(|n| n.analysis.clone())
                .unwrap_or_default();
            for _ in 0..self.config.n_candidates_per_round {
                let (success, score) = evaluate(&context_query);
                total_candidates += 1;

                let mut node = Node::new(
                    format!("Round {} candidate", round),
                    format!("// generated for round {}", round),
                    if success { score } else { 0.0 },
                );
                node.results.insert("success".into(), serde_json::Value::Bool(success));
                node.results.insert("score".into(), serde_json::Value::Number(
                    serde_json::Number::from_f64(score).unwrap_or(serde_json::Number::from(0)),
                ));

                let node_score = if success { score } else { 0.0 };
                node.score = node_score;

                self.database.add(node.clone());

                if node_score > best_score {
                    best_score = node_score;
                    best_node = Some(node);
                    rounds_without_improvement = 0;
                } else {
                    rounds_without_improvement += 1;
                }
            }

            if let Some(target) = self.config.target_score {
                if best_score >= target {
                    stopped_early = true;
                    break;
                }
            }

            if rounds_without_improvement >= self.config.patience {
                stopped_early = true;
                break;
            }

            if round % 10 == 0 {
                self.database.prune_bottom(50);
            }
        }

        EvolutionResult {
            success: total_candidates > 0,
            best_score,
            best_node,
            total_rounds: if stopped_early {
                self.config.max_rounds.min(
                    (total_candidates / self.config.n_candidates_per_round.max(1)) + 1,
                )
            } else {
                self.config.max_rounds
            },
            total_candidates,
            total_time_secs: start.elapsed().as_secs_f64(),
            stopped_early,
        }
    }
}
