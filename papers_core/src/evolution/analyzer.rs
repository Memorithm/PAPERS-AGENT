use crate::evolution::engineer::EngineerOutput;
use crate::llm::LlmClient;

const ANALYZER_SYSTEM: &str = "\
You are an expert scientific analyst. Distill experimental outcomes into structured, \
actionable insights. Be precise, critical, and constructive.";

const ANALYZER_PROMPT: &str = "\
## Candidate Program
Motivation: {motivation}
```
{program_summary}
```

## Experimental Results
Score: {score}
Success: {success}
Metrics: {metrics}
Runtime: {runtime_seconds}s

## Instructions
Analyze the experimental results above and produce a JSON object with:
- \"summary\": A concise 2-3 sentence summary.
- \"strengths\": List of what worked well.
- \"weaknesses\": List of what failed or underperformed.
- \"root_cause\": Your hypothesis for WHY the result turned out this way.
- \"actionable_insights\": Specific suggestions for the next iteration.
- \"novelty_assessment\": Score 0-10 and brief justification.";

/// Structured output from the Analyzer.
pub struct AnalysisOutput {
    pub summary: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub root_cause: String,
    pub actionable_insights: Vec<String>,
    pub novelty_score: f64,
    pub cognition_update: String,
}

/// Analyzes experimental results to guide future evolution rounds.
pub struct Analyzer {
    #[allow(dead_code)]
    model: String,
}

impl Analyzer {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
        }
    }

    /// LLM-powered analysis of a candidate's execution results.
    pub fn analyze(
        &self,
        llm: &LlmClient,
        motivation: &str,
        program: &str,
        result: &EngineerOutput,
    ) -> AnalysisOutput {
        let metrics_str = serde_json::to_string(&result.metrics).unwrap_or_default();

        let prompt = ANALYZER_PROMPT
            .replace("{motivation}", &motivation.chars().take(1500).collect::<String>())
            .replace("{program_summary}", &program.chars().take(2000).collect::<String>())
            .replace("{score}", &result.score.to_string())
            .replace("{success}", &result.success.to_string())
            .replace("{metrics}", &metrics_str.chars().take(1500).collect::<String>())
            .replace("{runtime_seconds}", &result.runtime_secs.to_string());

        match llm.generate_json(&prompt, Some(ANALYZER_SYSTEM)) {
            Ok(json) => {
                let extract_list = |key: &str| -> Vec<String> {
                    json.get(key)
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default()
                };

                let novelty = json
                    .get("novelty_assessment")
                    .and_then(|v| v.as_object())
                    .and_then(|o| o.get("score"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(5.0);

                AnalysisOutput {
                    summary: json.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    strengths: extract_list("strengths"),
                    weaknesses: extract_list("weaknesses"),
                    root_cause: json.get("root_cause").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    actionable_insights: extract_list("actionable_insights"),
                    novelty_score: novelty,
                    cognition_update: Self::build_cognition(
                        json.get("summary").and_then(|v| v.as_str()).unwrap_or(""),
                        &extract_list("actionable_insights"),
                    ),
                }
            }
            Err(_) => Self::heuristic_analysis(result),
        }
    }

    /// Build a structured cognition string from analysis output.
    pub fn build_cognition(summary: &str, insights: &[String]) -> String {
        let mut parts = vec![format!("Summary: {}", summary)];
        parts.push("Actionable Insights:".into());
        for (i, insight) in insights.iter().enumerate() {
            parts.push(format!("  {}. {}", i + 1, insight));
        }
        parts.join("\n")
    }

    /// Let the Analyzer store its output in cognition for future rounds.
    fn heuristic_analysis(result: &EngineerOutput) -> AnalysisOutput {
        let summary = format!(
            "Experiment {} with score {:.4}. {}",
            if result.success { "succeeded" } else { "failed" },
            result.score,
            result.error.as_deref().unwrap_or("No errors reported.")
        );
        let cognition = Self::build_cognition(
            &summary,
            &[
                if result.error.is_some() {
                    "Fix implementation issues and resubmit."
                } else {
                    "Consider architectural improvements for higher score."
                }
                .into(),
            ],
        );

        AnalysisOutput {
            summary,
            strengths: if result.success {
                vec!["Execution completed".into()]
            } else {
                vec![]
            },
            weaknesses: match &result.error {
                Some(e) => vec![format!("Error: {}", e)],
                None => vec!["Score below threshold".into()],
            },
            root_cause: result.error.clone().unwrap_or_else(|| "Insufficient data".into()),
            actionable_insights: vec![
                if result.error.is_some() {
                    "Fix implementation issues and resubmit."
                } else {
                    "Consider architectural improvements for higher score."
                }
                .into(),
            ],
            novelty_score: 5.0,
            cognition_update: cognition,
        }
    }
}
