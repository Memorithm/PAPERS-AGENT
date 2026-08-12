pub mod analyzer;
pub mod engineer;
pub mod researcher;

use std::time::Instant;

use crate::cognition::CognitionStore;
use crate::database::Database;
use crate::llm::LlmClient;
use crate::models::*;

pub use self::analyzer::{AnalysisOutput, Analyzer};
pub use self::engineer::{Engineer, EngineerOutput};
pub use self::researcher::{Researcher, ResearcherOutput};

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
            engineer: Engineer::new(3600).with_wasm(crate::wasm_executor::WasmConfig::default()),
        }
    }

    pub fn seed_cognition(&mut self, items: Vec<CognitionItem>) {
        self.cognition.add_batch(items);
    }

    /// Runs the Researcher -> Engineer -> Analyzer pipeline.
    ///
    /// A candidate contributes to selection only when its evaluator reports
    /// `success=true`. Structural/sandbox refusals may expose diagnostic scores,
    /// but those scores are never interpreted as empirical fitness.
    pub fn run_advanced(&mut self, llm: &LlmClient, task_description: &str) -> EvolutionResult {
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
            let cognition_owned: Vec<_> = self
                .cognition
                .retrieve(&context_query, self.config.n_cognition)
                .into_iter()
                .cloned()
                .collect();
            let cognition_refs: Vec<&CognitionItem> = cognition_owned.iter().collect();

            for i in 0..self.config.n_candidates_per_round {
                let parent_program = if i > 0 {
                    self.database
                        .best()
                        .map(|n| n.code.clone())
                        .unwrap_or_default()
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

                let result = self
                    .engineer
                    .execute(&output.program, None::<fn(&str) -> EngineerOutput>);
                let diagnostic_score =
                    Engineer::compute_fitness(&result.metrics, result.score, None);
                let fitness = if result.success {
                    diagnostic_score
                } else {
                    0.0
                };

                total_candidates += 1;

                let mut node =
                    Node::new(output.motivation.clone(), output.program.clone(), fitness);
                node.results
                    .insert("success".into(), serde_json::Value::Bool(result.success));
                node.results
                    .insert("fitness".into(), serde_json::json!(fitness));
                node.results.insert(
                    "diagnostic_score".into(),
                    serde_json::json!(diagnostic_score),
                );
                node.results.insert(
                    "runtime_secs".into(),
                    serde_json::json!(result.runtime_secs),
                );
                if let Some(ref err) = result.error {
                    node.results.insert("error".into(), serde_json::json!(err));
                }
                node.analysis = format!(
                    "fitness={fitness:.4}, diagnostic={diagnostic_score:.4}, success={}",
                    result.success
                );
                node.score = fitness;

                self.database.add(node.clone());

                if result.success && fitness > best_score {
                    best_score = fitness;
                    best_node = Some(node);
                    rounds_without_improvement = 0;
                } else {
                    rounds_without_improvement += 1;
                }

                let cognition_update = {
                    let analyzer = Analyzer::new(&self.config.sampling_policy);
                    let analysis =
                        analyzer.analyze(llm, &output.motivation, &output.program, &result);
                    analysis.cognition_update
                };

                if !cognition_update.is_empty() {
                    self.cognition.add(crate::models::CognitionItem::new(
                        cognition_update,
                        "analyzer".into(),
                        vec!["insight".into(), format!("round-{round}")],
                    ));
                }
            }

            if let Some(target) = self.config.target_score {
                if best_node.is_some() && best_score >= target {
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
            success: best_node.is_some(),
            best_score,
            best_node,
            total_rounds: if stopped_early {
                self.config
                    .max_rounds
                    .min((total_candidates / self.config.n_candidates_per_round.max(1)) + 1)
            } else {
                self.config.max_rounds
            },
            total_candidates,
            total_time_secs: start.elapsed().as_secs_f64(),
            stopped_early,
        }
    }

    /// Deterministic/test hook with an explicit evaluator supplied by the caller.
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
                let (success, raw_score) = evaluate(&context_query);
                let fitness = if success { raw_score } else { 0.0 };
                total_candidates += 1;

                let mut node = Node::new(
                    format!("Round {round} candidate"),
                    format!("// generated for round {round}"),
                    fitness,
                );
                node.results
                    .insert("success".into(), serde_json::Value::Bool(success));
                node.results
                    .insert("fitness".into(), serde_json::json!(fitness));
                node.results
                    .insert("diagnostic_score".into(), serde_json::json!(raw_score));
                node.score = fitness;

                self.database.add(node.clone());

                if success && fitness > best_score {
                    best_score = fitness;
                    best_node = Some(node);
                    rounds_without_improvement = 0;
                } else {
                    rounds_without_improvement += 1;
                }
            }

            if let Some(target) = self.config.target_score {
                if best_node.is_some() && best_score >= target {
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
            success: best_node.is_some(),
            best_score,
            best_node,
            total_rounds: if stopped_early {
                self.config
                    .max_rounds
                    .min((total_candidates / self.config.n_candidates_per_round.max(1)) + 1)
            } else {
                self.config.max_rounds
            },
            total_candidates,
            total_time_secs: start.elapsed().as_secs_f64(),
            stopped_early,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_evaluation_never_becomes_best_candidate() {
        let cfg = EvolutionConfig {
            max_rounds: 1,
            n_candidates_per_round: 1,
            patience: usize::MAX,
            ..EvolutionConfig::default()
        };
        let mut loop_ = EvolutionLoop::new(cfg);
        let result = loop_.run(|_| (false, 0.99));
        assert!(!result.success);
        assert_eq!(result.best_score, 0.0);
        assert!(result.best_node.is_none());
    }
}
