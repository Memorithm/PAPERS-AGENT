//! Bibliographic-only quantum source admission.
use papers_core::paper_registry::PaperEntry;
use std::collections::HashSet;

#[test]
fn quantum_catalogue_has_unique_sources_and_no_claimed_full_text() {
    let registry: Vec<PaperEntry> =
        serde_json::from_str(include_str!("../../papers_registry.json")).unwrap();
    assert_eq!(registry.len(), 4);
    let mut ids = HashSet::new();
    let mut titles = HashSet::new();
    for paper in &registry {
        assert!(ids.insert(&paper.id), "duplicate ID");
        assert!(titles.insert(&paper.title), "duplicate title");
        assert_eq!(
            paper.source.as_deref(),
            Some("external-bibliographic-reference-only")
        );
        assert!(paper
            .url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://")));
        assert!(paper.abstract_text.is_none());
        assert!(paper.key_insight.is_none());
        assert!(!paper.authors.is_empty());
    }
}

#[test]
fn all_four_books_have_deny_by_default_rights_and_matching_sources() {
    let registry: Vec<PaperEntry> =
        serde_json::from_str(include_str!("../../papers_registry.json")).unwrap();
    let permissions: serde_json::Value =
        serde_json::from_str(include_str!("../../catalog/quantum_access_and_rights.json")).unwrap();
    assert_eq!(
        permissions["schema"],
        "memorithm.science/bibliography-access-v1"
    );
    assert_eq!(permissions["scope"], "bibliographic_metadata_only");
    let policy = &permissions["data_policy"];
    for flag in [
        "pdf_bytes_stored",
        "full_text_extracted",
        "embeddings_generated",
        "training_launched",
    ] {
        assert_eq!(policy[flag], false, "unexpected policy flag: {flag}");
    }
    let entries = permissions["entries"].as_array().unwrap();
    assert_eq!(entries.len(), registry.len());
    let mut matched = HashSet::new();
    for record in entries {
        let id = record["id"].as_str().unwrap();
        assert!(matched.insert(id), "duplicate rights record");
        let paper = registry
            .iter()
            .find(|paper| paper.id == id)
            .expect("orphan rights record");
        assert_eq!(
            record["primary_reference_url"].as_str(),
            paper.url.as_deref()
        );
        assert_eq!(record["rights_clearance"], "not_established");
        assert_eq!(record["training_authorized"], false);
        assert_eq!(record["fulltext_indexing_authorized"], false);
        assert_eq!(record["redistribution_authorized"], false);
        assert_eq!(record["source_pdf_archived"], false);
        assert!(record["source_pdf_sha256"].is_null());
    }
    assert_eq!(matched.len(), registry.len());
    let vol2 = entries
        .iter()
        .find(|r| r["id"] == "isbn:9782759822867")
        .unwrap();
    assert!(vol2["primary_reference_url"]
        .as_str()
        .unwrap()
        .ends_with("Mecanique_Quantique_2.pdf"));
    assert_eq!(vol2["official_pdf_pages"], 704);
    let vol3 = entries
        .iter()
        .find(|r| r["id"] == "isbn:9782759823352")
        .unwrap();
    assert_eq!(vol3["official_pdf_pages"], 816);
    assert_eq!(vol3["pdf_ebook_isbn"], "9782759823369");
    assert!(vol3["primary_reference_url"]
        .as_str()
        .unwrap()
        .contains("laboutique.edpsciences.fr"));
}
