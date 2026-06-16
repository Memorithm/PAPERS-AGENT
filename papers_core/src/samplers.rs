use rand::Rng;

use crate::models::Node;

pub trait Sampler: Send + Sync {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node>;
}

pub struct GreedySampler;
impl Sampler for GreedySampler {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node> {
        let mut sorted: Vec<&Node> = nodes.iter().collect();
        sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).cloned().collect()
    }
}

pub struct RandomSampler;
impl Sampler for RandomSampler {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node> {
        let mut rng = rand::thread_rng();
        let n = n.min(nodes.len());
        let mut indices: Vec<usize> = (0..nodes.len()).collect();
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            let idx = rng.gen_range(0..indices.len());
            result.push(nodes[indices.remove(idx)].clone());
        }
        result
    }
}

pub struct UCB1Sampler {
    pub c: f64,
}
impl Sampler for UCB1Sampler {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node> {
        let n = n.min(nodes.len());
        if nodes.is_empty() {
            return Vec::new();
        }

        let total_visits: usize = nodes.iter().map(|n| n.visit_count).sum();
        let scored: Vec<&Node> = nodes.iter().filter(|n| n.visit_count > 0).collect();

        if scored.is_empty() {
            return RandomSampler.sample(nodes, n);
        }

        let min_score = scored.iter().map(|n| n.score).fold(f64::INFINITY, f64::min);
        let max_score = scored.iter().map(|n| n.score).fold(f64::NEG_INFINITY, f64::max);
        let score_range = if (max_score - min_score).abs() < 1e-10 {
            1.0
        } else {
            max_score - min_score
        };

        let mut ucb_values: Vec<(usize, f64)> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                if node.visit_count == 0 {
                    (i, f64::INFINITY)
                } else {
                    let normalized = (node.score - min_score) / score_range;
                    let exploration = self.c
                        * ((total_visits as f64).ln() / node.visit_count as f64).sqrt();
                    (i, normalized + exploration)
                }
            })
            .collect();

        ucb_values.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ucb_values
            .into_iter()
            .take(n)
            .map(|(i, _)| {
                let mut node = nodes[i].clone();
                node.visit_count += 1;
                node
            })
            .collect()
    }
}

pub struct IslandSampler {
    pub num_islands: usize,
    pub exploration_ratio: f64,
    pub exploitation_ratio: f64,
    islands: Vec<Vec<usize>>,
    current_island: usize,
    generations: usize,
}

impl IslandSampler {
    pub fn new(num_islands: usize) -> Self {
        Self {
            num_islands,
            exploration_ratio: 0.2,
            exploitation_ratio: 0.3,
            islands: vec![Vec::new(); num_islands],
            current_island: 0,
            generations: 0,
        }
    }
}

impl Sampler for IslandSampler {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node> {
        let n = n.min(nodes.len());
        if nodes.is_empty() {
            return Vec::new();
        }

        let mut rng = rand::thread_rng();
        let mut selected = Vec::new();

        for _ in 0..n {
            let r: f64 = rng.gen();
            if r < self.exploration_ratio {
                let idx = rng.gen_range(0..nodes.len());
                selected.push(nodes[idx].clone());
            } else {
                let mut sorted: Vec<&Node> = nodes.iter().collect();
                sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                selected.push(sorted[0].clone());
            }
        }
        selected
    }
}

pub fn create_sampler(name: &str) -> Box<dyn Sampler> {
    match name {
        "greedy" => Box::new(GreedySampler),
        "random" => Box::new(RandomSampler),
        "ucb1" => Box::new(UCB1Sampler { c: 1.414 }),
        "island" | "map_elites" => Box::new(IslandSampler::new(5)),
        _ => Box::new(GreedySampler),
    }
}
