use papers_core::engine::{
    AlgorithmDesc, AnalysisReport, Recommendation, Risk, SystemRequirements,
};
use papers_core::extraction::ExtractedDocument;
use papers_core::scientific_contract::{
    ClaimKind, ClaimState, ScientificBundle, SCIENTIFIC_BUNDLE_SCHEMA, SCIENTIFIC_CLAIM_SCHEMA,
};

fn report() -> AnalysisReport {
    AnalysisReport {
        document: ExtractedDocument {
            id: "PAPER-TEST-1".into(),
            title: "A deterministic scientific fixture".into(),
            authors: vec!["Researcher A".into()],
            publication_date: Some("2026-08-12".into()),
            source: "fixture://paper-test-1".into(),
            paper_url: None,
            github_url: None,
            abstract_text: Some("We report a tiled method that reduces latency.".into()),
            full_text: None,
            references: Vec::new(),
            parsed: None,
        },
        contributions: vec!["A tiled method reduces latency on the reported workload.".into()],
        executive_summary: "Fixture summary".into(),
        equations: Vec::new(),
        variables: Vec::new(),
        algorithms: vec![AlgorithmDesc {
            name: "TiledMethod".into(),
            complexity: Some("O(n)".into()),
            pseudocode: Some("for tile in input: process(tile)".into()),
        }],
        system_requirements: SystemRequirements {
            vram: None,
            ram: None,
            disk: None,
            latency: None,
            throughput: None,
            scalability: None,
        },
        risks: vec![Risk {
            level: "low".into(),
            description: "fixture".into(),
            mitigation: None,
        }],
        recommendation: Recommendation::Prototype,
        recommendation_justification: "fixture".into(),
        integration_score: 0.5,
        reproducibility_score: 0.5,
        impacted_modules: vec!["src/kernel.rs".into()],
        timestamp: "2026-08-12T00:00:00Z".into(),
        architectural_mapping: serde_json::json!({}),
        deep_analysis: serde_json::json!({}),
        experiment_plan: serde_json::json!({}),
        pseudo_code: serde_json::json!({}),
    }
}

#[test]
fn analysis_report_exports_versioned_valid_bundle() {
    let bundle = ScientificBundle::from_analysis_report(&report(), None).expect("bundle");
    bundle.validate().expect("valid bundle");

    assert_eq!(bundle.schema, SCIENTIFIC_BUNDLE_SCHEMA);
    assert_eq!(bundle.paper.id, "PAPER-TEST-1");
    assert_eq!(bundle.claims.len(), 2);
    assert!(bundle.proposals.is_empty());
    assert_eq!(
        bundle.provenance.extracted_content_scope,
        papers_core::scientific_contract::ContentScope::Abstract
    );

    assert_eq!(bundle.claims[0].schema, SCIENTIFIC_CLAIM_SCHEMA);
    assert_eq!(bundle.claims[0].kind, ClaimKind::Contribution);
    assert_eq!(bundle.claims[0].state, ClaimState::Reported);
    assert_eq!(bundle.claims[1].kind, ClaimKind::Method);
    assert_eq!(bundle.claims[1].state, ClaimState::Inferred);
    assert_eq!(bundle.claims[1].method.as_deref(), Some("TiledMethod"));
}

#[test]
fn conversion_is_replay_stable_for_same_analysis() {
    let a = ScientificBundle::from_analysis_report(&report(), None).expect("bundle a");
    let b = ScientificBundle::from_analysis_report(&report(), None).expect("bundle b");
    let a_json = serde_json::to_string(&a).expect("serialize a");
    let b_json = serde_json::to_string(&b).expect("serialize b");
    assert_eq!(a_json, b_json);
    assert_eq!(a.provenance.analysis_sha256, b.provenance.analysis_sha256);
    assert_eq!(a.claims[0].id, b.claims[0].id);
}

#[test]
fn model_identity_changes_trust_state_without_inventing_confidence() {
    use papers_core::scientific_contract::ModelProvenance;

    let bundle = ScientificBundle::from_analysis_report(
        &report(),
        Some(ModelProvenance {
            provider: "ollama".into(),
            model: "fixture-model".into(),
            config_sha256: None,
        }),
    )
    .expect("bundle");

    assert_eq!(bundle.claims[0].state, ClaimState::Inferred);
    assert!(bundle.claims.iter().all(|claim| claim.confidence.is_none()));
}
