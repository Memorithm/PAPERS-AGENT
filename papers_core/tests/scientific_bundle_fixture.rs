use papers_core::scientific_contract::ScientificBundle;

const FIXTURE: &str = include_str!("fixtures/scientific_bundle_v1.json");

#[test]
fn canonical_bundle_v1_round_trips_through_papers_types() {
    let bundle: ScientificBundle =
        serde_json::from_str(FIXTURE).expect("canonical scientific bundle v1 must deserialize");
    bundle
        .validate()
        .expect("canonical scientific bundle v1 must validate");

    assert_eq!(bundle.paper.id, "fixture-paper-1");
    assert_eq!(bundle.claims.len(), 2);
    assert!(bundle.proposals.is_empty());

    let serialized = serde_json::to_string(&bundle).expect("serialize canonical bundle");
    let reparsed: ScientificBundle =
        serde_json::from_str(&serialized).expect("reparse canonical bundle");
    assert_eq!(reparsed, bundle);
}
