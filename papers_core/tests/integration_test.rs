#[cfg(test)]
mod tests {
    use papers_core::models::{CognitionItem, EvolutionConfig, Node};
    use papers_core::cognition::CognitionStore;
    use papers_core::database::Database;
    use papers_core::samplers::{create_sampler, GreedySampler, RandomSampler, UCB1Sampler};
    use papers_core::Sampler;

    #[test]
    fn test_node_creation() {
        let node = Node::new("test".into(), "code".into(), 0.75);
        assert_eq!(node.motivation, "test");
        assert_eq!(node.score, 0.75);
        assert!(node.id.is_none());
    }

    #[test]
    fn test_database_add_and_sample() {
        let mut db = Database::new("greedy");
        for i in 0..10 {
            db.add(Node::new(format!("m{}", i), format!("c{}", i), i as f64 / 10.0));
        }
        assert_eq!(db.len(), 10);

        let sampled = db.sample(3);
        assert_eq!(sampled.len(), 3);
        assert!(sampled[0].score >= sampled[1].score);
    }

    #[test]
    fn test_database_best() {
        let mut db = Database::new("greedy");
        db.add(Node::new("low".into(), "c".into(), 0.1));
        db.add(Node::new("high".into(), "c".into(), 0.9));
        let best = db.best().unwrap();
        assert_eq!(best.score, 0.9);
    }

    #[test]
    fn test_database_stats() {
        let mut db = Database::new("greedy");
        db.add(Node::new("a".into(), "c".into(), 0.5));
        db.add(Node::new("b".into(), "c".into(), 1.0));
        let stats = db.stats();
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.max_score, 1.0);
    }

    #[test]
    fn test_database_prune() {
        let mut db = Database::new("greedy");
        for i in 0..20 {
            db.add(Node::new(format!("n{}", i), "c".into(), i as f64));
        }
        db.prune_bottom(10);
        assert!(db.len() <= 10);
    }

    #[test]
    fn test_greedy_sampler() {
        let nodes: Vec<Node> = (0..5)
            .map(|i| Node::new(format!("n{}", i), "c".into(), i as f64))
            .collect();
        let sampler = GreedySampler;
        let sampled = sampler.sample(&nodes, 3);
        assert_eq!(sampled.len(), 3);
        assert_eq!(sampled[0].score, 4.0);
    }

    #[test]
    fn test_random_sampler() {
        let nodes: Vec<Node> = (0..10)
            .map(|i| Node::new(format!("n{}", i), "c".into(), i as f64))
            .collect();
        let sampler = RandomSampler;
        let sampled = sampler.sample(&nodes, 5);
        assert_eq!(sampled.len(), 5);
    }

    #[test]
    fn test_ucb1_sampler() {
        let nodes: Vec<Node> = (0..5)
            .map(|i| {
                let mut n = Node::new(format!("n{}", i), "c".into(), i as f64 / 5.0);
                n.visit_count = i + 1;
                n
            })
            .collect();
        let sampler = UCB1Sampler { c: 1.414 };
        let sampled = sampler.sample(&nodes, 3);
        assert_eq!(sampled.len(), 3);
    }

    #[test]
    fn test_cognition_store() {
        let mut store = CognitionStore::new();
        store.add(CognitionItem::new(
            "Test knowledge about patterns".into(),
            "paper_1".into(),
            vec!["pattern".into()],
        ));
        store.add(CognitionItem::new(
            "Machine learning optimization".into(),
            "paper_2".into(),
            vec!["ml".into()],
        ));
        assert_eq!(store.len(), 2);

        let results = store.retrieve("pattern", 5);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_cognition_search_by_tag() {
        let mut store = CognitionStore::new();
        store.add(CognitionItem::new("content".into(), "src".into(), vec!["attention".into()]));
        store.add(CognitionItem::new("other".into(), "src2".into(), vec!["ml".into()]));

        let by_tag = store.search_by_tag("attention");
        assert_eq!(by_tag.len(), 1);
    }

    #[test]
    fn test_evolution_loop_basic() {
        use papers_core::evolution::EvolutionLoop;

        let config = EvolutionConfig {
            max_rounds: 3,
            sampling_policy: "greedy".into(),
            n_context_nodes: 2,
            n_candidates_per_round: 2,
            patience: 5,
            ..Default::default()
        };

        let mut loop_ = EvolutionLoop::new(config);

        let mut call_count = 0;
        let result = loop_.run(|_query| {
            call_count += 1;
            (true, 0.5 + call_count as f64 * 0.1)
        });

        assert!(result.success);
        assert!(result.best_score > 0.5);
        assert!(result.total_candidates >= 2);
    }

    #[test]
    fn test_create_sampler() {
        let s = create_sampler("greedy");
        let nodes = vec![Node::new("n".into(), "c".into(), 0.5)];
        let sampled = s.sample(&nodes, 1);
        assert_eq!(sampled.len(), 1);
    }

    #[test]
    fn test_evolution_config_default() {
        let config = EvolutionConfig::default();
        assert_eq!(config.max_rounds, 50);
        assert_eq!(config.sampling_policy, "greedy");
    }

    #[test]
    fn test_cognition_item_new() {
        let item = CognitionItem::new("content".into(), "source".into(), vec!["tag".into()]);
        assert!(item.id.is_some());
        assert_eq!(item.content, "content");
        assert_eq!(item.tags, vec!["tag"]);
    }

    #[test]
    fn test_database_is_empty() {
        let db = Database::new("greedy");
        assert!(db.is_empty());
    }

    #[test]
    fn test_cognition_retrieve_empty() {
        let store = CognitionStore::new();
        let results = store.retrieve("query", 5);
        assert!(results.is_empty());
    }
}
