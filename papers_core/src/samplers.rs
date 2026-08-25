use rand::Rng;

use crate::models::Node;

pub trait Sampler: Send + Sync {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node>;
}

pub struct GreedySampler;
impl Sampler for GreedySampler {
    fn sample(&self, nodes: &[Node], n: usize) -> Vec<Node> {
        let mut sorted: Vec<&Node> = nodes.iter().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
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
        let max_score = scored
            .iter()
            .map(|n| n.score)
            .fold(f64::NEG_INFINITY, f64::max);
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
                    let exploration =
                        self.c * ((total_visits as f64).ln() / node.visit_count as f64).sqrt();
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
}

impl IslandSampler {
    pub fn new(num_islands: usize) -> Self {
        Self {
            num_islands,
            exploration_ratio: 0.2,
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
                sorted.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
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
        other => {
            eprintln!("⚠️  Sampler inconnu '{}', fallback vers greedy", other);
            Box::new(GreedySampler)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: usize, score: f64, visits: usize) -> Node {
        let mut n = Node::new(
            format!("motivation {id}"),
            format!("fn n{id}() {{}}"),
            score,
        );
        n.id = Some(id);
        n.visit_count = visits;
        n
    }

    #[test]
    fn greedy_returns_nodes_sorted_by_descending_score() {
        let nodes = vec![node(0, 0.2, 1), node(1, 0.9, 1), node(2, 0.5, 1)];
        let picked = GreedySampler.sample(&nodes, 2);
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].id, Some(1));
        assert_eq!(picked[1].id, Some(2));
    }

    #[test]
    fn greedy_n_larger_than_pool_returns_all() {
        let nodes = vec![node(0, 0.1, 0), node(1, 0.2, 0)];
        assert_eq!(GreedySampler.sample(&nodes, 10).len(), 2);
    }

    #[test]
    fn ucb1_prioritizes_unvisited_nodes() {
        // Un nœud jamais visité a une valeur UCB infinie : il doit passer en tête.
        let nodes = vec![node(0, 0.99, 100), node(1, 0.01, 0), node(2, 0.98, 50)];
        let picked = UCB1Sampler { c: 1.414 }.sample(&nodes, 1);
        assert_eq!(picked.len(), 1);
        assert_eq!(
            picked[0].id,
            Some(1),
            "le nœud non visité doit être exploré"
        );
    }

    #[test]
    fn ucb1_exploration_decays_as_visits_grow() {
        // Deux nœuds de même score ; l'un très visité, l'autre peu :
        // le moins visité obtient la meilleure valeur d'exploration.
        let s = UCB1Sampler { c: 1.414 };
        let nodes = vec![node(0, 0.5, 1000), node(1, 0.5, 3)];
        let picked = s.sample(&nodes, 1);
        assert_eq!(picked[0].id, Some(1));
    }

    #[test]
    fn ucb1_exploits_best_node_when_all_visited_evenly() {
        let s = UCB1Sampler { c: 0.1 }; // exploration faible : exploitation dominante
        let mut nodes: Vec<Node> = (0..5).map(|i| node(i as usize, 0.1, 10)).collect();
        nodes[3].score = 0.95;

        // Sur 20 tirages successifs avec mise à jour des visites,
        // le meilleur nœud doit dominer largement.
        let mut best_picks = 0;
        for _ in 0..20 {
            let picked = s.sample(&nodes, 1);
            if picked[0].id == Some(3) {
                best_picks += 1;
            }
            for p in picked {
                if let Some(id) = p.id {
                    nodes[id].visit_count += 1;
                }
            }
        }
        assert!(
            best_picks >= 15,
            "exploitation attendue du meilleur nœud, obtenu {best_picks}/20"
        );
    }

    #[test]
    fn ucb1_with_no_visits_falls_back_to_random_without_crash() {
        let s = UCB1Sampler { c: 1.414 };
        let nodes = vec![node(0, 0.1, 0), node(1, 0.2, 0), node(2, 0.3, 0)];
        let picked = s.sample(&nodes, 2);
        assert_eq!(picked.len(), 2);
        // IDs uniques : pas de doublon dans un tirage sans remise implicite.
        let ids: std::collections::HashSet<_> = picked.iter().map(|n| n.id).collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn ucb1_empty_pool_yields_empty_result() {
        assert!(UCB1Sampler { c: 1.0 }.sample(&[], 3).is_empty());
    }

    #[test]
    fn random_sampler_respects_bounds_and_uniqueness() {
        let nodes: Vec<Node> = (0..6).map(|i| node(i, i as f64 / 10.0, 1)).collect();
        for _ in 0..10 {
            let picked = RandomSampler.sample(&nodes, 3);
            assert_eq!(picked.len(), 3);
            let ids: std::collections::HashSet<_> = picked.iter().map(|n| n.id).collect();
            assert_eq!(ids.len(), 3, "tirages uniques attendus");
        }
    }

    #[test]
    fn island_sampler_stays_within_bounds() {
        let nodes: Vec<Node> = (0..4).map(|i| node(i, i as f64 / 8.0, 1)).collect();
        let island = IslandSampler::new(3);
        for _ in 0..20 {
            let picked = island.sample(&nodes, 5.min(nodes.len()));
            assert!(!picked.is_empty());
            assert!(picked.iter().all(|p| nodes.iter().any(|n| n.id == p.id)));
        }
        assert_eq!(island.num_islands, 3);
        assert!((island.exploration_ratio - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn create_sampler_known_names_and_unknown_fallback() {
        for name in ["greedy", "random", "ucb1", "island", "map_elites"] {
            let _ = create_sampler(name); // ne panique pas
        }
        let unknown = create_sampler("inconnu-au-bataillon");
        // Le fallback est greedy : il trie par score décroissant.
        let nodes = vec![node(0, 0.1, 0), node(1, 0.7, 0)];
        assert_eq!(unknown.sample(&nodes, 1)[0].id, Some(1));
    }
}
