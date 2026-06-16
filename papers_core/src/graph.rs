use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphPattern {
    pub id: String,
    pub pattern_type: String,
    pub description: String,
    pub significance: f64,
    pub frequency: usize,
}

pub struct GraphMiner {
    graph: DiGraph<String, String>,
    node_types: HashMap<NodeIndex, String>,
}

impl GraphMiner {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_types: HashMap::new(),
        }
    }

    pub fn add_paper(&mut self, paper_id: &str, title: &str) -> NodeIndex {
        let idx = self.graph.add_node(title.to_string());
        self.node_types.insert(idx, "paper".into());
        idx
    }

    pub fn add_tag(&mut self, paper: NodeIndex, tag: &str) -> NodeIndex {
        let idx = self.graph.add_node(tag.to_string());
        self.node_types.insert(idx, "tag".into());
        self.graph.add_edge(paper, idx, "has_tag".into());
        idx
    }

    pub fn add_relation(&mut self, from: NodeIndex, to: NodeIndex, relation: &str) {
        self.graph.add_edge(from, to, relation.to_string());
    }

    pub fn mine(&self) -> Vec<GraphPattern> {
        let mut patterns = Vec::new();

        // Star patterns
        for node in self.graph.node_indices() {
            let degree = self.graph.neighbors(node).count();
            if degree >= 3 {
                patterns.push(GraphPattern {
                    id: format!("star_{:?}", node.index()),
                    pattern_type: "star".into(),
                    description: format!(
                        "Star pattern: {:?} connects to {} nodes",
                        self.graph.node_weight(node).unwrap_or(&"?".into()),
                        degree
                    ),
                    significance: (degree as f64).ln() / 10.0,
                    frequency: degree,
                });
            }
        }

        // Bridge patterns
        let mut bridges = HashSet::new();
        for edge in self.graph.edge_indices() {
            let (u, v) = self.graph.edge_endpoints(edge).unwrap();
            let u_deg = self.graph.neighbors(u).count();
            let v_deg = self.graph.neighbors(v).count();
            if u_deg == 1 || v_deg == 1 {
                bridges.insert((u, v));
            }
        }
        for (u, v) in bridges {
            patterns.push(GraphPattern {
                id: format!("bridge_{:?}_{:?}", u.index(), v.index()),
                pattern_type: "bridge".into(),
                description: "Bridge connecting subgraphs".into(),
                significance: 0.5,
                frequency: 1,
            });
        }

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
