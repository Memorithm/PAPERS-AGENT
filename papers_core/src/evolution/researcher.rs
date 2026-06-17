use crate::llm::LlmClient;
use crate::models::{CognitionItem, Node};

const RESEARCHER_SYSTEM: &str = "\
You are an expert Rust programmer. Your ONLY job is to write complete, \
compilable Rust code that solves the given task. Always respond with a JSON \
object containing a 'program' field with working code and a 'motivation' field \
explaining your approach.";

const RESEARCHER_PROMPT: &str = "\
## Task
{task_description}

## Knowledge
{cognition_items}

## Prior Results
{context_nodes}

## Requirements
1. Write a SINGLE Rust function that solves the task
2. The code must be COMPLETE (all imports, all helper functions, all logic)
3. The code must be self-contained
4. Use only the Rust standard library
5. Return ONLY valid JSON: {{\"program\": \"...\", \"motivation\": \"...\"}}
6. The program field must contain the FULL source code as a string

Respond with JSON:
{{\"motivation\": \"explain your approach in 1-2 sentences\", \
\"program\": \"fn solve(input: &[i32]) -> Vec<i32> {{ input.to_vec() }}\"}}";

const DIFF_RESEARCHER_PROMPT: &str = "\
## Task Description
{task_description}

## Parent Program (to modify)
```
{parent_program}
```

## Retrieved Domain Knowledge (Cognition)
{cognition_items}

## Prior Experiment Context
{context_nodes}

## Instructions
You are modifying the parent program above. Produce localized, targeted changes. \
Respond as a JSON object with:
- \"motivation\": Explanation of what you are changing and why.
- \"program\": The COMPLETE modified program (not just the diff).
- \"diff_summary\": Concise description of the changes made.";

pub struct ResearcherOutput {
    pub motivation: String,
    pub program: String,
    pub diff_summary: String,
}

pub struct Researcher {
    #[allow(dead_code)]
    model: String,
}

impl Researcher {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
        }
    }

    pub fn generate(
        &self,
        llm: &LlmClient,
        task_description: &str,
        context_nodes: &[Node],
        cognition_items: &[&CognitionItem],
        parent_program: &str,
        use_diff: bool,
    ) -> ResearcherOutput {
        let cognition_text = Self::format_cognition(cognition_items);
        let context_text = Self::format_context(context_nodes);

        let prompt = if use_diff && !parent_program.is_empty() {
            DIFF_RESEARCHER_PROMPT
                .replace("{task_description}", task_description)
                .replace("{parent_program}", &parent_program.chars().take(8000).collect::<String>())
                .replace("{cognition_items}", &cognition_text)
                .replace("{context_nodes}", &context_text)
        } else {
            RESEARCHER_PROMPT
                .replace("{task_description}", task_description)
                .replace("{cognition_items}", &cognition_text)
                .replace("{context_nodes}", &context_text)
        };

        match llm.generate_json(&prompt, Some(RESEARCHER_SYSTEM)) {
            Ok(json) => {
                let program = json.get("program").and_then(|v| v.as_str()).unwrap_or("")
                    .to_string();
                let motivation = json.get("motivation").and_then(|v| v.as_str()).unwrap_or("")
                    .to_string();
                let diff_summary = json.get("diff_summary").and_then(|v| v.as_str()).unwrap_or("")
                    .to_string();

                let cleaned = Self::clean_program(&program);
                if Self::is_valid(&cleaned) {
                    ResearcherOutput {
                        motivation,
                        program: cleaned,
                        diff_summary,
                    }
                } else {
                    Self::fallback_generation(task_description)
                }
            }
            Err(_) => Self::fallback_generation(task_description),
        }
    }

    fn is_valid(program: &str) -> bool {
        program.contains("fn ") || program.contains("struct ") || program.contains("impl ") || program.contains("use ")
    }

    fn clean_program(program: &str) -> String {
        let mut s = program.trim().to_string();
        if s.starts_with("```") {
            let lines: Vec<&str> = s.lines().collect();
            let start = if lines.first().is_some_and(|l| l.starts_with("```")) { 1 } else { 0 };
            let end = if lines.last().is_some_and(|l| l.trim().starts_with("```")) {
                lines.len().saturating_sub(1)
            } else {
                lines.len()
            };
            s = lines[start..end].join("\n").trim().to_string();
        }
        s
    }

    fn format_cognition(items: &[&CognitionItem]) -> String {
        if items.is_empty() {
            return "(Aucune connaissance de domaine disponible)".to_string();
        }
        let mut lines = Vec::new();
        for (i, entry) in items.iter().enumerate() {
            lines.push(format!("### Cognition {} (source: {})", i + 1, entry.source));
            lines.push(entry.content.chars().take(1500).collect());
            lines.push(String::new());
        }
        lines.join("\n")
    }

    fn format_context(nodes: &[Node]) -> String {
        if nodes.is_empty() {
            return "(Aucun contexte experimental disponible - premier essai)".to_string();
        }
        let mut lines = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            let id_str = node.id.map(|id| id.to_string()).unwrap_or_else(|| "?".into());
            lines.push(format!("### Experiment {} ({}) - Score: {:.4}", i + 1, id_str, node.score));
            lines.push(format!("Motivation: {}", node.motivation.chars().take(300).collect::<String>()));
            if !node.analysis.is_empty() {
                lines.push(format!("Analysis: {}", node.analysis.chars().take(300).collect::<String>()));
            }
            if !node.results.is_empty() {
                if let Ok(json) = serde_json::to_string(&node.results) {
                    lines.push(format!("Results: {}", json.chars().take(500).collect::<String>()));
                }
            }
            lines.push(String::new());
        }
        lines.join("\n")
    }

    fn fallback_generation(task_description: &str) -> ResearcherOutput {
        let task_lower = task_description.to_lowercase();
        let (motivation, program) = if task_lower.contains("circle") || task_lower.contains("pack") {
            ("Hexagonal grid packing baseline".to_string(), r#"/// Place n circles in a unit square using hexagonal packing.
pub fn place_circles(n: usize) -> Vec<(f64, f64, f64)> {
    let cols = (n as f64 * 2.0 / 3.0_f64.sqrt()).sqrt().ceil() as usize;
    let r = 1.0 / (2.0 * cols as f64);
    let row_h = r * 3.0_f64.sqrt();
    let mut result = Vec::new();
    let mut placed = 0;
    let mut row = 0;
    while placed < n {
        let n_cols = if row % 2 == 0 { cols } else { cols.saturating_sub(1) };
        for col in 0..n_cols {
            if placed >= n { break; }
            let x = if row % 2 == 0 {
                r * (1.0 + 2.0 * col as f64)
            } else {
                r * (2.0 + 2.0 * col as f64)
            };
            let y = r + row as f64 * row_h;
            if y + r <= 1.0 && x - r >= 0.0 && x + r <= 1.0 {
                result.push((x, y, r));
                placed += 1;
            }
        }
        row += 1;
    }
    result
}"#.to_string())
        } else if task_lower.contains("sort") || task_lower.contains("optimize") {
            ("Baseline sort optimization".to_string(), r#"/// Sort and return optimized data.
pub fn optimize(data: &mut [i32]) -> Vec<i32> {
    data.sort_unstable();
    data.to_vec()
}"#.to_string())
        } else {
            (format!("Generic solver for: {}", &task_description.chars().take(80).collect::<String>()),
             format!("/// Solution for: {}\npub fn solve(data: &[i32]) -> Vec<i32> {{\n    data.to_vec()\n}}", 
                 &task_description.chars().take(60).collect::<String>()))
        };

        ResearcherOutput {
            motivation,
            program,
            diff_summary: String::new(),
        }
    }
}
