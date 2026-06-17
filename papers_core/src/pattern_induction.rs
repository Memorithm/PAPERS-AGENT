use crate::models::Node;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PatternInduction {
    pub patterns: Vec<Pattern>,
    min_occurrences: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Pattern {
    pub id: String,
    pub signature: String,
    pub description: String,
    pub source_nodes: Vec<usize>,
    pub success_rate: f64,
    pub usage_count: usize,
}

impl PatternInduction {
    pub fn new(min_occurrences: usize) -> Self {
        Self {
            patterns: Vec::new(),
            min_occurrences,
        }
    }

    pub fn induce(&mut self, history: &[Node], min_score: f64) -> Vec<&Pattern> {
        let qualified: Vec<&Node> = history.iter().filter(|n| n.score >= min_score).collect();
        if qualified.len() < self.min_occurrences {
            return Vec::new();
        }

        self.extract_parent_child(&qualified);
        self.extract_structural(&qualified);
        self.extract_convergent(history);

        self.patterns
            .iter()
            .filter(|p| p.success_rate >= min_score)
            .collect()
    }

    fn extract_parent_child(&mut self, nodes: &[&Node]) {
        let mut parent_map: HashMap<usize, Vec<&Node>> = HashMap::new();
        for node in nodes {
            for p in &node.parent {
                parent_map.entry(*p).or_default().push(node);
            }
        }

        for (_pid, children) in parent_map {
            if children.len() < 2 {
                continue;
            }
            let scores: Vec<f64> = children.iter().map(|c| c.score).collect();
            let mean = scores.iter().sum::<f64>() / scores.len() as f64;
            let improvement = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - scores.iter().cloned().fold(f64::INFINITY, f64::min);

            if improvement > 0.1 {
                let ids: Vec<usize> = children.iter().filter_map(|c| c.id).collect();
                self.patterns.push(Pattern {
                    id: format!("parent_imp_{}", ids.first().unwrap_or(&0)),
                    signature: format!("parent->child improvement {:.3}", improvement),
                    description: format!(
                        "Parent yields improvements. Mean: {:.3}, max improvement: {:.3}",
                        mean, improvement
                    ),
                    source_nodes: ids,
                    success_rate: mean,
                    usage_count: children.len(),
                });
            }
        }
    }

    fn extract_structural(&mut self, nodes: &[&Node]) {
        let mut blocks: HashMap<String, (usize, f64)> = HashMap::new();
        for node in nodes {
            let code_blocks = extract_code_blocks(&node.code);
            for block in code_blocks {
                let entry = blocks.entry(block).or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += node.score;
            }
        }

        for (block, (freq, total_score)) in blocks {
            if freq >= self.min_occurrences {
                let avg = total_score / freq as f64;
                if avg >= 0.5 {
                    self.patterns.push(Pattern {
                        id: format!("struct_{:x}", block.len()),
                        signature: block.chars().take(80).collect(),
                        description: format!("Recurring {freq}x, avg score {avg:.3}"),
                        source_nodes: Vec::new(),
                        success_rate: avg,
                        usage_count: freq,
                    });
                }
            }
        }
    }

    fn extract_convergent(&mut self, history: &[Node]) {
        let scores: Vec<f64> = history.iter().map(|n| n.score).collect();
        for (i, w) in scores.windows(3).enumerate() {
            if w[0] <= w[1] && w[1] <= w[2] {
                let ids: Vec<usize> = history[i..i + 3]
                    .iter()
                    .filter_map(|n| n.id)
                    .collect();
                self.patterns.push(Pattern {
                    id: format!("conv_{}", ids.first().unwrap_or(&0)),
                    signature: "monotonic_improvement".into(),
                    description: format!("Sustained improvement: {:.3} -> {:.3}", w[0], w[2]),
                    source_nodes: ids,
                    success_rate: (w[0] + w[1] + w[2]) / 3.0,
                    usage_count: 3,
                });
            }
        }
    }
}

fn extract_code_blocks(code: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
            blocks.push(trimmed.chars().take(120).collect());
        }
    }
    if blocks.is_empty() {
        blocks.push(code.chars().take(120).collect());
    }
    blocks
}
