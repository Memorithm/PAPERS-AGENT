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
            engineer: Engineer::new(3600)
                .with_wasm(crate::wasm_executor::WasmConfig::default()),
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
            let cognition_owned: Vec<_> = self.cognition.retrieve(&context_query, self.config.n_cognition)
                .into_iter().cloned().collect();
            let cognition_refs: Vec<&CognitionItem> = cognition_owned.iter().collect();

            for i in 0..self.config.n_candidates_per_round {
                let parent_program = if i > 0 {
                    self.database.best().map(|n| n.code.clone()).unwrap_or_default()
                } else {
                    String::new()
                };
                let has_parent = !parent_program.is_empty();

                let output = self.researcher.generate(
                    llm,
                    task_description,
                    &context_nodes,
                    &cognition_refs,
                    &parent_program,
                    has_parent,
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

                // Run Analyzer on each candidate to generate insights
                let motivation = output.motivation.clone();
                let program = output.program.clone();
                let cognition_update = {
                    let analyzer = Analyzer::new(&self.config.sampling_policy);
                    let analysis = analyzer.analyze(
                        llm,
                        &motivation,
                        &program,
                        &result,
                    );
                    analysis.cognition_update
                };

                // Store analysis insights in cognition for future rounds
                if !cognition_update.is_empty() {
                    self.cognition.add(crate::models::CognitionItem::new(
                        cognition_update,
                        "analyzer".into(),
                        vec!["insight".into(), format!("round-{}", round)],
                    ));
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
