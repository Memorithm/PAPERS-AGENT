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
                .replace(
                    "{parent_program}",
                    &parent_program.chars().take(8000).collect::<String>(),
                )
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
                let program = json
                    .get("program")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let motivation = json
                    .get("motivation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let diff_summary = json
                    .get("diff_summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
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
        program.contains("fn ")
            || program.contains("struct ")
            || program.contains("impl ")
            || program.contains("use ")
    }

    fn clean_program(program: &str) -> String {
        let mut s = program.trim().to_string();
        if s.starts_with("```") {
            let lines: Vec<&str> = s.lines().collect();
            let start = if lines.first().is_some_and(|l| l.starts_with("```")) {
                1
            } else {
                0
            };
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
            lines.push(format!(
                "### Cognition {} (source: {})",
                i + 1,
                entry.source
            ));
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
            let id_str = node
                .id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "?".into());
            lines.push(format!(
                "### Experiment {} ({}) - Score: {:.4}",
                i + 1,
                id_str,
                node.score
            ));
            lines.push(format!(
                "Motivation: {}",
                node.motivation.chars().take(300).collect::<String>()
            ));
            if !node.analysis.is_empty() {
                lines.push(format!(
                    "Analysis: {}",
                    node.analysis.chars().take(300).collect::<String>()
                ));
            }
            if !node.results.is_empty() {
                if let Ok(json) = serde_json::to_string(&node.results) {
                    lines.push(format!(
                        "Results: {}",
                        json.chars().take(500).collect::<String>()
                    ));
                }
            }
            lines.push(String::new());
        }
        lines.join("\n")
    }

    fn fallback_generation(task_description: &str) -> ResearcherOutput {
        let task_lower = task_description.to_lowercase();
        let (motivation, program) = if task_lower.contains("transformer")
            || task_lower.contains("attention")
            || task_lower.contains("llm")
        {
            ("Multi-head attention baseline".to_string(), r#"/// Simple multi-head attention forward pass.
pub fn attention(q: &[f32], k: &[f32], v: &[f32], d_k: usize) -> Vec<f32> {
    let n = q.len() / d_k;
    let mut scores = vec![0.0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut dot = 0.0;
            for kk in 0..d_k {
                dot += q[i * d_k + kk] * k[j * d_k + kk];
            }
            scores[i * n + j] = dot / (d_k as f32).sqrt();
        }
    }
    // softmax + weighted sum (simplified)
    let mut output = vec![0.0f32; n * d_k];
    for i in 0..n {
        let mut max_s = scores[i * n..(i + 1) * n].iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut sum_s = 0.0;
        for j in 0..n {
            let exp_val = (scores[i * n + j] - max_s).exp();
            sum_s += exp_val;
        }
        for j in 0..n {
            let attn = (scores[i * n + j] - max_s).exp() / sum_s;
            for kk in 0..d_k {
                output[i * d_k + kk] += attn * v[j * d_k + kk];
            }
        }
    }
    output
}"#.to_string())
        } else if task_lower.contains("embedding")
            || task_lower.contains("semantic")
            || task_lower.contains("vector")
        {
            (
                "Simple embedding projection".to_string(),
                r#"/// Project tokens into embedding space.
pub fn embed(tokens: &[u32], weights: &[f32], vocab_size: usize, d_model: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; tokens.len() * d_model];
    for (i, &tok) in tokens.iter().enumerate() {
        let tok = tok as usize % vocab_size;
        let src = tok * d_model;
        let dst = i * d_model;
        for j in 0..d_model {
            out[dst + j] = weights[src + j];
        }
    }
    out
}"#
                .to_string(),
            )
        } else if task_lower.contains("rnn")
            || task_lower.contains("lstm")
            || task_lower.contains("gru")
        {
            (
                "Simple RNN cell forward pass".to_string(),
                r#"/// Single RNN cell step.
pub fn rnn_step(x: &[f32], h_prev: &[f32], w_ih: &[f32], w_hh: &[f32]) -> Vec<f32> {
    let hidden_size = h_prev.len();
    let mut h = vec![0.0f32; hidden_size];
    for i in 0..hidden_size {
        let mut sum = 0.0;
        for j in 0..x.len() { sum += w_ih[i * x.len() + j] * x[j]; }
        for j in 0..hidden_size { sum += w_hh[i * hidden_size + j] * h_prev[j]; }
        h[i] = sum.tanh();
    }
    h
}"#
                .to_string(),
            )
        } else if task_lower.contains("conv")
            || task_lower.contains("cnn")
            || task_lower.contains("vision")
        {
            (
                "Simple 2D convolution baseline".to_string(),
                r#"/// 2D convolution (single channel, no padding).
pub fn conv2d(input: &[f32], kernel: &[f32], h: usize, w: usize, k: usize) -> Vec<f32> {
    let oh = h - k + 1;
    let ow = w - k + 1;
    let mut out = vec![0.0f32; oh * ow];
    for i in 0..oh {
        for j in 0..ow {
            let mut sum = 0.0;
            for ki in 0..k {
                for kj in 0..k {
                    let ii = i + ki;
                    let jj = j + kj;
                    sum += input[ii * w + jj] * kernel[ki * k + kj];
                }
            }
            out[i * ow + j] = sum;
        }
    }
    out
}"#
                .to_string(),
            )
        } else if task_lower.contains("graph")
            || task_lower.contains("gnn")
            || task_lower.contains("message")
        {
            (
                "Simple GNN message passing layer".to_string(),
                r#"/// Graph message passing between nodes.
pub fn message_passing(node_feats: &[f32], adj: &[f32], n: usize, d: usize) -> Vec<f32> {
    let mut updated = vec![0.0f32; n * d];
    for i in 0..n {
        let mut sum = vec![0.0f32; d];
        for j in 0..n {
            if adj[i * n + j] > 0.0 {
                for k in 0..d {
                    sum[k] += node_feats[j * d + k];
                }
            }
        }
        let deg = (0..n).filter(|&j| adj[i * n + j] > 0.0).count().max(1);
        for k in 0..d {
            updated[i * d + k] = sum[k] / deg as f32;
        }
    }
    updated
}"#
                .to_string(),
            )
        } else if task_lower.contains("sort") || task_lower.contains("optimize") {
            (
                "Baseline sort optimization".to_string(),
                r#"/// Sort and return optimized data.
pub fn optimize(data: &mut [i32]) -> Vec<i32> {
    data.sort_unstable();
    data.to_vec()
}"#
                .to_string(),
            )
        } else if task_lower.contains("circle") || task_lower.contains("pack") {
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
            let x = if row % 2 == 0 { r * (1.0 + 2.0 * col as f64) } else { r * (2.0 + 2.0 * col as f64) };
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
        } else if task_lower.contains("agent")
            || task_lower.contains("reinforce")
            || task_lower.contains("rl")
        {
            ("Simple Q-learning agent baseline".to_string(), r#"/// Tabular Q-learning update step.
pub struct QAgent { q_table: Vec<f64>, n_actions: usize, lr: f64, gamma: f64 }

impl QAgent {
    pub fn new(n_states: usize, n_actions: usize) -> Self {
        Self { q_table: vec![0.0; n_states * n_actions], n_actions, lr: 0.1, gamma: 0.99 }
    }
    pub fn update(&mut self, s: usize, a: usize, r: f64, s_next: usize) {
        let max_next = (0..self.n_actions).map(|a2| self.q_table[s_next * self.n_actions + a2]).fold(f64::NEG_INFINITY, f64::max);
        let target = r + self.gamma * max_next;
        let idx = s * self.n_actions + a;
        self.q_table[idx] += self.lr * (target - self.q_table[idx]);
    }
    pub fn act(&self, s: usize, epsilon: f64) -> usize {
        let start = s * self.n_actions;
        (0..self.n_actions).max_by(|&a, &b| self.q_table[start + a].total_cmp(&self.q_table[start + b])).unwrap_or(0)
    }
}"#.to_string())
        } else if task_lower.contains("search") || task_lower.contains("mcts") {
            (
                "Monte Carlo Tree Search baseline".to_string(),
                r#"/// Simple MCTS node.
pub struct MctsNode {
    pub visits: u32, pub value: f64, pub children: Vec<MctsNode>,
}

impl MctsNode {
    pub fn new() -> Self { Self { visits: 0, value: 0.0, children: Vec::new() } }
    pub fn ucb1(&self, total: u32, c: f64) -> f64 {
        if self.visits == 0 { return f64::INFINITY; }
        self.value / self.visits as f64 + c * (total as f64).ln() / self.visits as f64
    }
}"#
                .to_string(),
            )
        } else {
            (format!("Generic solver for: {}", &task_description.chars().take(80).collect::<String>()),
             format!("/// Solution for: {}\npub fn solve(data: &[i32]) -> Vec<i32> {{\n    data.to_vec()\n}}\n\npub fn process(data: &mut [f64]) -> f64 {{\n    data.iter().sum::<f64>() / data.len() as f64\n}}",
                 &task_description.chars().take(60).collect::<String>()))
        };

        ResearcherOutput {
            motivation,
            program,
            diff_summary: String::new(),
        }
    }
}
