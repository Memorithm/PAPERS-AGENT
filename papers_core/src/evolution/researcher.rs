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
1. Write a Rust function that solves the task AND an executable fn main()
2. The code must be COMPLETE (all imports, all helper functions, all logic)
3. The code must be self-contained
4. Use only the Rust standard library
5. Return ONLY valid JSON: {\"program\": \"...\", \"motivation\": \"...\"}
6. The program field must contain the FULL source code as a string
7. The FIRST statement of main() must call the declared candidate function on literal bounded development inputs (optionally let result = candidate(...)), then print its output
8. This harness is a runtime diagnostic, NOT a task oracle or empirical fitness claim

Respond with JSON:
{\"motivation\": \"explain your approach in 1-2 sentences\", \
\"program\": \"fn solve(input: &[i32]) -> Vec<i32> { input.to_vec() } fn main() { let result = solve(&[1, 2]); println!(\\\"{:?}\\\", result); }\"}";

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
Preserve an executable fn main() whose FIRST statement calls the declared candidate on literal bounded \
development example; do not return a helper-only program or infer scientific \
fitness from this diagnostic harness. \
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
    /// Deterministic development candidate with a real calling harness.
    /// It supplies no task-oracle or scientific authority.
    pub fn development_fallback(task_description: &str) -> ResearcherOutput {
        Self::fallback_generation(task_description)
    }

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
        // Conservative syntax preflight, not scientific qualification. The first
        // statement must invoke a declared nonempty function, not mention it
        // inside a comment, string, closure, branch or no-op macro.
        if program.len() > 64 * 1024 {
            return false;
        }
        let Ok(file) = syn::parse_file(program) else {
            return false;
        };
        let functions: Vec<_> = file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(function) => Some(function),
                _ => None,
            })
            .collect();
        let Some(main) = functions
            .iter()
            .find(|function| function.sig.ident == "main")
        else {
            return false;
        };
        if !main.sig.inputs.is_empty() {
            return false;
        }
        let call = match main.block.stmts.first() {
            Some(syn::Stmt::Expr(syn::Expr::Call(call), _)) => call,
            Some(syn::Stmt::Local(local)) => match local.init.as_ref() {
                Some(init) if init.diverge.is_none() => match init.expr.as_ref() {
                    syn::Expr::Call(call) => call,
                    _ => return false,
                },
                _ => return false,
            },
            _ => return false,
        };
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return false;
        };
        let Some(name) = path.path.get_ident() else {
            return false;
        };
        name != "main"
            && functions
                .iter()
                .any(|function| function.sig.ident == *name && !function.block.stmts.is_empty())
            && call.args.iter().all(Self::literal_probe_input)
    }

    fn literal_probe_input(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Lit(_) => true,
            syn::Expr::Array(array) => array.elems.iter().all(Self::literal_probe_input),
            syn::Expr::Tuple(tuple) => tuple.elems.iter().all(Self::literal_probe_input),
            syn::Expr::Reference(reference) => Self::literal_probe_input(&reference.expr),
            syn::Expr::Unary(unary) if matches!(&unary.op, syn::UnOp::Neg(_)) => {
                Self::literal_probe_input(&unary.expr)
            }
            _ => false,
        }
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
            (format!("Generic solver for: {}", task_description.chars().take(80).collect::<String>()),
             format!("/// Solution for: {}\npub fn solve(data: &[i32]) -> Vec<i32> {{\n    data.to_vec()\n}}\n\npub fn process(data: &mut [f64]) -> f64 {{\n    data.iter().sum::<f64>() / data.len() as f64\n}}",
                 task_description.chars().take(60).collect::<String>()))
        };

        // Preserve each existing fallback algorithm and actually invoke it.
        // These small development inputs are diagnostics, never locked data or
        // an independent task oracle; Engineer assigns zero empirical fitness.
        let invocation = match motivation.as_str() {
            "Multi-head attention baseline" => "println!(\"{:?}\", attention(&[1.0, 0.0], &[1.0, 0.0], &[2.0, 3.0], 2));",
            "Simple embedding projection" => "println!(\"{:?}\", embed(&[0], &[1.0, 2.0], 1, 2));",
            "Simple RNN cell forward pass" => "println!(\"{:?}\", rnn_step(&[1.0], &[0.0], &[1.0], &[0.0]));",
            "Simple 2D convolution baseline" => "println!(\"{:?}\", conv2d(&[1.0, 2.0, 3.0, 4.0], &[1.0], 2, 2, 1));",
            "Simple GNN message passing layer" => "println!(\"{:?}\", message_passing(&[1.0, 2.0], &[0.0, 1.0, 1.0, 0.0], 2, 1));",
            "Baseline sort optimization" => "println!(\"{:?}\", optimize(&mut [3, 1, 2]));",
            "Hexagonal grid packing baseline" => "println!(\"{:?}\", place_circles(1));",
            "Simple Q-learning agent baseline" => "let mut agent = QAgent::new(2, 2); agent.update(0, 0, 1.0, 1); println!(\"{}\", agent.act(0, 0.0));",
            "Monte Carlo Tree Search baseline" => "println!(\"{}\", MctsNode::new().ucb1(1, 1.0));",
            _ => "println!(\"{:?}\", solve(&[1, 2]));",
        };
        let program = format!("{program}\n\nfn development_probe() {{ {invocation} }}\nfn main() {{ development_probe(); }}\n");

        ResearcherOutput {
            motivation,
            program,
            diff_summary: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_only_generation_is_not_an_executable_candidate() {
        assert!(!Researcher::is_valid("fn solve(x: i32) -> i32 { x }"));
        assert!(Researcher::is_valid(
            "fn solve(x: i32) -> i32 { x } fn main() { let result = solve(1); println!(\"{}\", result); }"
        ));
    }

    #[test]
    fn empty_noncalling_or_conditional_entrypoints_are_refused() {
        for main in [
            "fn main() {}",
            "fn main() { println!(\"hello\"); }",
            "fn main() { println!(\"solve(1)\"); }",
            "fn main() { if false { solve(1); } }",
            "fn main() { let _unused = || solve(1); }",
            "fn main() { return; solve(1); }",
            "fn main() { let _ = solve({ std::process::exit(0); }); }",
        ] {
            assert!(
                !Researcher::is_valid(&format!("fn solve(x: i32) -> i32 {{ x }} {main}")),
                "{main}"
            );
        }
    }

    #[test]
    fn every_fallback_preserves_its_algorithm_and_adds_a_calling_harness() {
        for (task, helper) in [
            ("attention", "pub fn attention"),
            ("embedding", "pub fn embed"),
            ("rnn", "pub fn rnn_step"),
            ("conv", "pub fn conv2d"),
            ("graph", "pub fn message_passing"),
            ("sort", "pub fn optimize"),
            ("circle", "pub fn place_circles"),
            ("agent", "pub struct QAgent"),
            ("search", "pub struct MctsNode"),
            ("generic", "pub fn solve"),
        ] {
            let output = Researcher::fallback_generation(task);
            assert!(output.program.contains(helper), "{task}");
            assert!(Researcher::is_valid(&output.program), "{task}");
            assert!(
                output
                    .program
                    .contains("fn main() { development_probe(); }"),
                "{task}"
            );
        }
    }
}
