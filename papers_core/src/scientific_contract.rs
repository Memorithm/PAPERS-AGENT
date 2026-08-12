//! Versioned scientific interchange contracts for PAPERS -> CCOS Research Lab -> RSI.
//! A paper, an LLM inference, and an empirical observation are intentionally
//! represented as different trust classes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::AnalysisReport;

pub const SCIENTIFIC_BUNDLE_SCHEMA: &str = "memorithm.science/bundle-v1";
pub const SCIENTIFIC_CLAIM_SCHEMA: &str = "memorithm.science/claim-v1";
pub const EXPERIMENT_PROPOSAL_SCHEMA: &str = "memorithm.science/experiment-proposal-v1";
pub const EXPERIMENT_RESULT_SCHEMA: &str = "memorithm.science/experiment-result-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOrigin {
    SourceSpan,
    AnalysisField,
    ModelInference,
    Experiment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentScope {
    FullText,
    Abstract,
    Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProvenance {
    pub provider: String,
    pub model: String,
    pub config_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub paper_id: String,
    pub source: String,
    /// Hash of the exact extracted content PAPERS saw, not necessarily PDF bytes.
    pub extracted_content_sha256: String,
    pub extracted_content_scope: ContentScope,
    pub analysis_sha256: String,
    pub generator: String,
    pub generator_version: String,
    pub generated_at: String,
    pub model: Option<ModelProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceSpan {
    pub origin: EvidenceOrigin,
    pub locator: String,
    pub section: Option<String>,
    pub page: Option<u32>,
    pub text: Option<String>,
    pub text_sha256: Option<String>,
}

impl EvidenceSpan {
    pub fn analysis_field(locator: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            origin: EvidenceOrigin::AnalysisField,
            locator: locator.into(),
            section: None,
            page: None,
            text_sha256: Some(sha256_hex(text.as_bytes())),
            text: Some(text),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Contribution,
    Method,
    Result,
    Limitation,
    Assumption,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Reported,
    Inferred,
    Reproduced,
    PartiallyReproduced,
    Contradicted,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScientificClaim {
    pub schema: String,
    pub id: String,
    pub paper_id: String,
    pub kind: ClaimKind,
    pub statement: String,
    pub state: ClaimState,
    pub evidence: Vec<EvidenceSpan>,
    pub assumptions: Vec<String>,
    pub method: Option<String>,
    pub algorithm: Option<String>,
    pub baseline: Option<String>,
    pub dataset: Option<String>,
    pub metrics: Vec<String>,
    pub expected_effect: Option<String>,
    pub reported_effect: Option<String>,
    pub limitations: Vec<String>,
    pub falsification_criteria: Vec<String>,
    /// None means PAPERS has no defensible numeric confidence.
    pub confidence: Option<f64>,
    pub provenance: Provenance,
}

impl ScientificClaim {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCIENTIFIC_CLAIM_SCHEMA {
            return Err(format!("unsupported claim schema: {}", self.schema));
        }
        if self.id.trim().is_empty() || self.paper_id.trim().is_empty() {
            return Err("claim id and paper_id must be non-empty".into());
        }
        if self.statement.trim().is_empty() {
            return Err("claim statement must be non-empty".into());
        }
        if let Some(c) = self.confidence {
            if !c.is_finite() || !(0.0..=1.0).contains(&c) {
                return Err("claim confidence must be finite and in [0, 1]".into());
            }
        }
        self.provenance.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaperIdentity {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub publication_date: Option<String>,
    pub source: String,
    pub paper_url: Option<String>,
    pub github_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceLimits {
    pub timeout_seconds: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub max_output_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperimentProposal {
    pub schema: String,
    pub id: String,
    pub hypothesis: String,
    pub claim_ids: Vec<String>,
    pub target_component: String,
    pub intervention: String,
    pub baseline: String,
    pub metrics: Vec<String>,
    pub expected_direction: Option<String>,
    pub expected_effect: Option<String>,
    pub workload: Option<String>,
    pub seed: u64,
    pub repetitions: u32,
    pub acceptance_criteria: Vec<String>,
    pub rejection_criteria: Vec<String>,
    pub resource_limits: ResourceLimits,
    pub safety_constraints: Vec<String>,
    pub provenance: Provenance,
}

impl ExperimentProposal {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != EXPERIMENT_PROPOSAL_SCHEMA {
            return Err(format!("unsupported proposal schema: {}", self.schema));
        }
        if self.id.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.target_component.trim().is_empty()
            || self.intervention.trim().is_empty()
            || self.baseline.trim().is_empty()
        {
            return Err(
                "proposal id, hypothesis, target, intervention and baseline are required".into(),
            );
        }
        if self.repetitions == 0 {
            return Err("proposal repetitions must be >= 1".into());
        }
        self.provenance.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Planned,
    Running,
    Passed,
    Failed,
    Inconclusive,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricObservation {
    pub value: f64,
    pub unit: Option<String>,
    pub uncertainty: Option<f64>,
    pub samples: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperimentResult {
    pub schema: String,
    pub id: String,
    pub proposal_id: String,
    pub status: ExperimentStatus,
    pub metrics: BTreeMap<String, MetricObservation>,
    pub evidence: Vec<EvidenceSpan>,
    pub artifacts: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub provenance: Provenance,
}

impl ExperimentResult {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != EXPERIMENT_RESULT_SCHEMA {
            return Err(format!("unsupported result schema: {}", self.schema));
        }
        if self.id.trim().is_empty() || self.proposal_id.trim().is_empty() {
            return Err("result id and proposal_id must be non-empty".into());
        }
        for (name, observation) in &self.metrics {
            if name.trim().is_empty() || !observation.value.is_finite() {
                return Err("metric names must be non-empty and values finite".into());
            }
            if let Some(u) = observation.uncertainty {
                if !u.is_finite() || u < 0.0 {
                    return Err("metric uncertainty must be finite and non-negative".into());
                }
            }
        }
        self.provenance.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScientificBundle {
    pub schema: String,
    pub paper: PaperIdentity,
    pub claims: Vec<ScientificClaim>,
    /// Intentionally empty until a typed planner supplies baseline, metrics,
    /// workload and falsification criteria. PAPERS must not invent them.
    pub proposals: Vec<ExperimentProposal>,
    pub provenance: Provenance,
}

impl ScientificBundle {
    pub fn from_analysis_report(
        report: &AnalysisReport,
        model: Option<ModelProvenance>,
    ) -> Result<Self, String> {
        let model_backed = model.is_some();
        let provenance = Provenance::from_analysis_report(report, model)?;
        let paper = PaperIdentity {
            id: report.document.id.clone(),
            title: report.document.title.clone(),
            authors: report.document.authors.clone(),
            publication_date: report.document.publication_date.clone(),
            source: report.document.source.clone(),
            paper_url: report.document.paper_url.clone(),
            github_url: report.document.github_url.clone(),
        };

        let mut claims = Vec::new();
        for (index, contribution) in report.contributions.iter().enumerate() {
            let statement = contribution.trim();
            if statement.is_empty() {
                continue;
            }
            claims.push(ScientificClaim {
                schema: SCIENTIFIC_CLAIM_SCHEMA.into(),
                id: deterministic_id("claim", &report.document.id, statement, index),
                paper_id: report.document.id.clone(),
                kind: ClaimKind::Contribution,
                statement: statement.to_string(),
                state: if model_backed {
                    ClaimState::Inferred
                } else {
                    ClaimState::Reported
                },
                evidence: vec![EvidenceSpan::analysis_field(
                    format!("analysis.contributions[{index}]"),
                    statement,
                )],
                assumptions: Vec::new(),
                method: None,
                algorithm: None,
                baseline: None,
                dataset: None,
                metrics: Vec::new(),
                expected_effect: None,
                reported_effect: None,
                limitations: Vec::new(),
                falsification_criteria: Vec::new(),
                confidence: None,
                provenance: provenance.clone(),
            });
        }

        for (index, algorithm) in report.algorithms.iter().enumerate() {
            let name = algorithm.name.trim();
            if name.is_empty() || is_placeholder_algorithm(name, algorithm.pseudocode.as_deref()) {
                continue;
            }
            let statement = format!("Method described by analysis: {name}");
            let mut evidence_text = name.to_string();
            if let Some(complexity) = algorithm.complexity.as_deref() {
                evidence_text.push_str(" | complexity: ");
                evidence_text.push_str(complexity);
            }
            if let Some(pseudocode) = algorithm.pseudocode.as_deref() {
                evidence_text.push_str(" | pseudocode: ");
                evidence_text.push_str(pseudocode);
            }
            claims.push(ScientificClaim {
                schema: SCIENTIFIC_CLAIM_SCHEMA.into(),
                id: deterministic_id("method", &report.document.id, name, index),
                paper_id: report.document.id.clone(),
                kind: ClaimKind::Method,
                statement,
                state: ClaimState::Inferred,
                evidence: vec![EvidenceSpan::analysis_field(
                    format!("analysis.algorithms[{index}]"),
                    evidence_text,
                )],
                assumptions: Vec::new(),
                method: Some(name.to_string()),
                algorithm: algorithm.pseudocode.clone(),
                baseline: None,
                dataset: None,
                metrics: Vec::new(),
                expected_effect: None,
                reported_effect: None,
                limitations: Vec::new(),
                falsification_criteria: Vec::new(),
                confidence: None,
                provenance: provenance.clone(),
            });
        }

        let bundle = Self {
            schema: SCIENTIFIC_BUNDLE_SCHEMA.into(),
            paper,
            claims,
            proposals: Vec::new(),
            provenance,
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCIENTIFIC_BUNDLE_SCHEMA {
            return Err(format!("unsupported bundle schema: {}", self.schema));
        }
        if self.paper.id.trim().is_empty() || self.paper.title.trim().is_empty() {
            return Err("paper id and title must be non-empty".into());
        }
        self.provenance.validate()?;
        for claim in &self.claims {
            claim.validate()?;
            if claim.paper_id != self.paper.id {
                return Err(format!("claim {} has wrong paper_id", claim.id));
            }
        }
        for proposal in &self.proposals {
            proposal.validate()?;
        }
        Ok(())
    }
}

impl Provenance {
    pub fn from_analysis_report(
        report: &AnalysisReport,
        model: Option<ModelProvenance>,
    ) -> Result<Self, String> {
        let (content, scope) = if let Some(text) = report.document.full_text.as_deref() {
            (text.as_bytes().to_vec(), ContentScope::FullText)
        } else if let Some(text) = report.document.abstract_text.as_deref() {
            (text.as_bytes().to_vec(), ContentScope::Abstract)
        } else {
            let metadata = serde_json::to_vec(&(
                &report.document.id,
                &report.document.title,
                &report.document.authors,
                &report.document.publication_date,
                &report.document.source,
            ))
            .map_err(|e| format!("cannot serialize source metadata: {e}"))?;
            (metadata, ContentScope::Metadata)
        };
        let analysis = serde_json::to_vec(report)
            .map_err(|e| format!("cannot serialize analysis report: {e}"))?;
        Ok(Self {
            paper_id: report.document.id.clone(),
            source: report.document.source.clone(),
            extracted_content_sha256: sha256_hex(&content),
            extracted_content_scope: scope,
            analysis_sha256: sha256_hex(&analysis),
            generator: "papers_core::scientific_contract".into(),
            generator_version: env!("CARGO_PKG_VERSION").into(),
            // Reusing the source analysis timestamp makes conversion replayable.
            generated_at: report.timestamp.clone(),
            model,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.paper_id.trim().is_empty()
            || self.source.trim().is_empty()
            || self.generator.trim().is_empty()
            || self.generator_version.trim().is_empty()
            || self.generated_at.trim().is_empty()
        {
            return Err("provenance identity fields must be non-empty".into());
        }
        if !is_sha256_hex(&self.extracted_content_sha256) || !is_sha256_hex(&self.analysis_sha256) {
            return Err("provenance hashes must be lowercase SHA-256 hex".into());
        }
        if let Some(model) = &self.model {
            if model.provider.trim().is_empty() || model.model.trim().is_empty() {
                return Err("model provider and model must be non-empty".into());
            }
            if let Some(hash) = &model.config_sha256 {
                if !is_sha256_hex(hash) {
                    return Err("model config hash must be lowercase SHA-256 hex".into());
                }
            }
        }
        Ok(())
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn deterministic_id(prefix: &str, paper_id: &str, statement: &str, ordinal: usize) -> String {
    let digest = sha256_hex(format!("{prefix}\0{paper_id}\0{ordinal}\0{statement}").as_bytes());
    format!("{prefix}-{}", &digest[..24])
}

fn is_placeholder_algorithm(name: &str, pseudocode: Option<&str>) -> bool {
    let lower = name.to_lowercase();
    lower.contains("heuristique")
        || lower.contains("heuristic")
        || pseudocode.is_some_and(|p| p.contains("ProcessPaper"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> Provenance {
        Provenance {
            paper_id: "paper-1".into(),
            source: "fixture".into(),
            extracted_content_sha256: sha256_hex(b"source"),
            extracted_content_scope: ContentScope::FullText,
            analysis_sha256: sha256_hex(b"analysis"),
            generator: "test".into(),
            generator_version: "1".into(),
            generated_at: "2026-08-12T00:00:00Z".into(),
            model: None,
        }
    }

    #[test]
    fn evidence_hashes_exact_text() {
        let span = EvidenceSpan::analysis_field("analysis.x", "hello");
        let expected = sha256_hex(b"hello");
        assert_eq!(span.text_sha256.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn claim_rejects_invalid_confidence() {
        let claim = ScientificClaim {
            schema: SCIENTIFIC_CLAIM_SCHEMA.into(),
            id: "claim-1".into(),
            paper_id: "paper-1".into(),
            kind: ClaimKind::Contribution,
            statement: "x".into(),
            state: ClaimState::Reported,
            evidence: Vec::new(),
            assumptions: Vec::new(),
            method: None,
            algorithm: None,
            baseline: None,
            dataset: None,
            metrics: Vec::new(),
            expected_effect: None,
            reported_effect: None,
            limitations: Vec::new(),
            falsification_criteria: Vec::new(),
            confidence: Some(1.1),
            provenance: provenance(),
        };
        assert!(claim.validate().is_err());
    }

    #[test]
    fn proposal_requires_repetition() {
        let proposal = ExperimentProposal {
            schema: EXPERIMENT_PROPOSAL_SCHEMA.into(),
            id: "exp-1".into(),
            hypothesis: "A beats B".into(),
            claim_ids: vec!["claim-1".into()],
            target_component: "component".into(),
            intervention: "A".into(),
            baseline: "B".into(),
            metrics: vec!["latency".into()],
            expected_direction: Some("lower".into()),
            expected_effect: None,
            workload: None,
            seed: 42,
            repetitions: 0,
            acceptance_criteria: Vec::new(),
            rejection_criteria: Vec::new(),
            resource_limits: ResourceLimits::default(),
            safety_constraints: Vec::new(),
            provenance: provenance(),
        };
        assert!(proposal.validate().is_err());
    }

    #[test]
    fn ids_are_stable_and_domain_separated() {
        let a = deterministic_id("claim", "p", "same", 0);
        let b = deterministic_id("claim", "p", "same", 0);
        let c = deterministic_id("method", "p", "same", 0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
