use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphPattern {
    pub id: String,
    pub pattern_type: String,
    pub description: String,
    pub significance: f64,
    pub frequency: usize,
}

/// Deterministic PAPERS-local graph index.
///
/// The previous implementation mirrored paper/tag facts into SciRust's
/// `KnowledgeGraph`, which forced an absolute build-time SciRust dependency.
/// PAPERS only needs a small paper↔tag index here; richer knowledge-graph work is
/// delegated across the versioned SciRust/CCOS Research Lab runtime boundary.
pub struct GraphMiner {
    graph: DiGraph<String, String>,
    node_types: HashMap<NodeIndex, String>,
    paper_nodes: HashMap<NodeIndex, String>,
    papers_by_tag: HashMap<String, BTreeSet<String>>,
}

impl Default for GraphMiner {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphMiner {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_types: HashMap::new(),
            paper_nodes: HashMap::new(),
            papers_by_tag: HashMap::new(),
        }
    }

    pub fn add_paper(&mut self, paper_id: &str, title: &str) -> NodeIndex {
        let idx = self.graph.add_node(title.to_string());
        self.node_types.insert(idx, "paper".into());
        self.paper_nodes.insert(idx, paper_id.to_string());
        idx
    }

    pub fn add_tag(&mut self, paper_id: &str, paper_node: NodeIndex, tag: &str) -> NodeIndex {
        let idx = self.graph.add_node(tag.to_string());
        self.node_types.insert(idx, "tag".into());
        self.graph.add_edge(paper_node, idx, "has_tag".into());

        // Prefer the stable paper id already bound to the node. Keep the
        // explicit argument for API compatibility, but never silently associate
        // a different id with an existing node.
        let stable_id = self
            .paper_nodes
            .get(&paper_node)
            .map(String::as_str)
            .unwrap_or(paper_id);
        self.papers_by_tag
            .entry(tag.to_string())
            .or_default()
            .insert(stable_id.to_string());
        idx
    }

    pub fn add_relation(&mut self, from: NodeIndex, to: NodeIndex, relation: &str) {
        self.graph.add_edge(from, to, relation.to_string());
    }

    /// Return stable paper identifiers for a tag in deterministic lexical order.
    pub fn query_papers_by_tag(&self, tag: &str) -> Vec<String> {
        self.papers_by_tag
            .get(tag)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn mine(&self) -> Vec<GraphPattern> {
        let mut patterns = Vec::new();
        for node in self.graph.node_indices() {
            let degree = self.graph.neighbors(node).count();
            if degree >= 3 {
                patterns.push(GraphPattern {
                    id: format!("star_{}", node.index()),
                    pattern_type: "star".into(),
                    description: format!("Star pattern: connects to {degree} nodes"),
                    significance: (degree as f64).ln() / 10.0,
                    frequency: degree,
                });
            }
        }

        let mut bridges = HashSet::new();
        for edge in self.graph.edge_indices() {
            if let Some((u, v)) = self.graph.edge_endpoints(edge) {
                if self.graph.neighbors(u).count() == 1 || self.graph.neighbors(v).count() == 1 {
                    bridges.insert((u, v));
                }
            }
        }
        for (u, v) in bridges {
            patterns.push(GraphPattern {
                id: format!("bridge_{}_{}", u.index(), v.index()),
                pattern_type: "bridge".into(),
                description: "Bridge connecting subgraphs".into(),
                significance: 0.5,
                frequency: 1,
            });
        }
        patterns.sort_by(|a, b| a.id.cmp(&b.id));
        patterns
    }

    pub fn stats(&self) -> GraphStats {
        GraphStats {
            nodes: self.graph.node_count(),
            edges: self.graph.edge_count(),
            papers: self.node_types.values().filter(|t| *t == "paper").count(),
            tags: self.node_types.values().filter(|t| *t == "tag").count(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphStats {
    pub nodes: usize,
    pub edges: usize,
    pub papers: usize,
    pub tags: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_lookup_returns_stable_paper_ids() {
        let mut miner = GraphMiner::new();
        let b = miner.add_paper("paper-b", "B");
        let a = miner.add_paper("paper-a", "A");
        miner.add_tag("paper-b", b, "attention");
        miner.add_tag("paper-a", a, "attention");
        assert_eq!(
            miner.query_papers_by_tag("attention"),
            vec!["paper-a".to_string(), "paper-b".to_string()]
        );
    }

    #[test]
    fn bound_node_identity_wins_over_mismatched_argument() {
        let mut miner = GraphMiner::new();
        let paper = miner.add_paper("paper-real", "Paper");
        miner.add_tag("paper-wrong", paper, "rust");
        assert_eq!(
            miner.query_papers_by_tag("rust"),
            vec!["paper-real".to_string()]
        );
    }
}
