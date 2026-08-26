use std::fs;
use std::path::Path;

use crate::engine::AnalysisReport;

fn frontmatter(report: &AnalysisReport) -> String {
    let doc = &report.document;
    let mut md = String::from("---\n");
    md.push_str(&format!("id: {}\n", doc.id));
    md.push_str(&format!("title: {}\n", doc.title));
    md.push_str(&format!("authors: {}\n", doc.authors.join(", ")));
    md.push_str(&format!("source: {}\n", doc.source));
    md.push_str(&format!(
        "date: {}\n",
        doc.publication_date.as_deref().unwrap_or("N/A")
    ));
    md.push_str(&format!(
        "integration_score: {:.2}\n",
        report.integration_score
    ));
    md.push_str(&format!(
        "reproducibility_score: {:.2}\n",
        report.reproducibility_score
    ));
    md.push_str(&format!(
        "recommendation: {}\n",
        report.recommendation.label()
    ));
    md.push_str(&format!(
        "github: {}\n",
        doc.github_url.as_deref().unwrap_or("N/A")
    ));
    md.push_str(&format!(
        "paper_url: {}\n",
        doc.paper_url.as_deref().unwrap_or("N/A")
    ));
    md.push_str("---\n\n");
    md
}

fn executive_section(report: &AnalysisReport) -> String {
    let doc = &report.document;
    let mut md = String::from("# Résumé Exécutif\n\n");
    md.push_str(&report.executive_summary);
    md.push_str("\n\n");

    if let Some(ref abs) = doc.abstract_text {
        md.push_str("## Abstract\n\n");
        md.push_str(abs);
        md.push_str("\n\n");
    }
    md
}

fn contributions_section(report: &AnalysisReport) -> String {
    let mut md = String::from("## Contributions Scientifiques\n\n");
    for c in &report.contributions {
        md.push_str(&format!("- {}\n", c));
    }
    md.push('\n');
    md
}

fn equations_section(report: &AnalysisReport) -> String {
    if report.equations.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Équations\n\n");
    for eq in &report.equations {
        md.push_str(&format!("- `{}`\n", eq));
    }
    md.push('\n');
    md
}

fn variables_section(report: &AnalysisReport) -> String {
    if report.variables.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Variables\n\n");
    md.push_str("| Variable | Signification |\n");
    md.push_str("|----------|---------------|\n");
    for (name, meaning) in &report.variables {
        md.push_str(&format!("| {} | {} |\n", name, meaning));
    }
    md.push('\n');
    md
}

fn algorithms_section(report: &AnalysisReport) -> String {
    if report.algorithms.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Algorithmes\n\n");
    for algo in &report.algorithms {
        md.push_str(&format!("### {}\n\n", algo.name));
        if let Some(ref complexity) = algo.complexity {
            md.push_str(&format!("- **Complexité**: {}\n", complexity));
        }
        if let Some(ref pseudo) = algo.pseudocode {
            md.push_str("\n```text\n");
            md.push_str(pseudo);
            md.push_str("\n```\n\n");
        }
    }
    md
}

fn system_section(report: &AnalysisReport) -> String {
    let sys = &report.system_requirements;
    let mut md = String::from("## Analyse Système\n\n");
    md.push_str("| Ressource | Valeur |\n");
    md.push_str("|-----------|--------|\n");
    for (label, value) in [
        ("VRAM", sys.vram.as_deref()),
        ("RAM", sys.ram.as_deref()),
        ("I/O disque", sys.disk.as_deref()),
        ("Latence", sys.latency.as_deref()),
        ("Débit", sys.throughput.as_deref()),
        ("Scalabilité", sys.scalability.as_deref()),
    ] {
        md.push_str(&format!("| {} | {} |\n", label, value.unwrap_or("N/A")));
    }
    md.push('\n');
    md
}

fn risks_section(report: &AnalysisReport) -> String {
    if report.risks.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Risques\n\n");
    for risk in &report.risks {
        md.push_str(&format!("- **{}**: {}", risk.level, risk.description));
        if let Some(ref mitigation) = risk.mitigation {
            md.push_str(&format!(" (Atténuation: {})", mitigation));
        }
        md.push('\n');
    }
    md.push('\n');
    md
}

fn references_section(report: &AnalysisReport) -> String {
    if report.document.references.is_empty() {
        return String::new();
    }
    let mut md = String::from("## Références\n\n");
    for (i, r) in report.document.references.iter().enumerate() {
        md.push_str(&format!("[{}] {}\n", i + 1, r));
    }
    md.push('\n');
    md
}

fn architectural_mapping_section(report: &AnalysisReport) -> String {
    if report.architectural_mapping == serde_json::Value::Null {
        return String::new();
    }
    let mut md = String::from("## Cartographie Architecturale\n\n");
    if let Some(obj) = report.architectural_mapping.as_object() {
        for (pillar, keywords) in obj {
            let Some(arr) = keywords.as_array() else {
                continue;
            };
            if arr.is_empty() {
                continue;
            }
            md.push_str(&format!("### {}\n", pillar));
            for kw in arr.iter().filter_map(|kw| kw.as_str()) {
                md.push_str(&format!("- {}\n", kw));
            }
            md.push('\n');
        }
    }
    md
}

fn deep_analysis_section(report: &AnalysisReport) -> String {
    if report.deep_analysis == serde_json::Value::Null {
        return String::new();
    }
    let mut md = String::from("## Analyse Approfondie\n\n");
    match &report.deep_analysis {
        serde_json::Value::String(s) => {
            md.push_str(s);
            md.push_str("\n\n");
        }
        obj @ serde_json::Value::Object(_) => {
            if let Some(summary) = obj.get("summary").and_then(|v| v.as_str()) {
                md.push_str(summary);
                md.push_str("\n\n");
            }
        }
        _ => {}
    }
    md
}

fn experiment_plan_section(report: &AnalysisReport) -> String {
    if report.experiment_plan == serde_json::Value::Null {
        return String::new();
    }
    let mut md = String::from("## Plan d'Expérience\n\n");
    match &report.experiment_plan {
        serde_json::Value::String(s) => {
            md.push_str(s);
            md.push_str("\n\n");
        }
        obj @ serde_json::Value::Object(_) => {
            if let Some(steps) = obj.get("steps").and_then(|v| v.as_array()) {
                for (i, step) in steps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| s.as_str().map(|s| (i, s)))
                {
                    md.push_str(&format!("{}. {}\n", i + 1, step));
                }
                md.push('\n');
            }
        }
        _ => {}
    }
    md
}

fn pseudo_code_section(report: &AnalysisReport) -> String {
    if report.pseudo_code == serde_json::Value::Null {
        return String::new();
    }
    let code = match &report.pseudo_code {
        serde_json::Value::String(s) => Some(s.as_str()),
        obj @ serde_json::Value::Object(_) => obj.get("code").and_then(|v| v.as_str()),
        _ => None,
    };
    let Some(code) = code else {
        return String::new();
    };
    format!("## Pseudo-code\n\n```text\n{code}\n```\n\n")
}

fn llm_warnings_section(report: &AnalysisReport) -> String {
    if report.llm_warnings.is_empty() {
        return String::new();
    }
    let mut md = String::from("## ⚠️ Avertissements LLM\n\n");
    md.push_str(
        "Sections possiblement incomplètes : le LLM n'a pas pu fournir \
                 une réponse exploitable pour les analyses suivantes.\n\n",
    );
    for w in &report.llm_warnings {
        md.push_str(&format!("- {}\n", w));
    }
    md.push('\n');
    md
}

fn recommendation_section(report: &AnalysisReport) -> String {
    let mut md = String::from("## Recommandation\n\n");
    md.push_str(&format!("**{}**\n\n", report.recommendation.label()));
    md.push_str(&report.recommendation_justification);
    md.push_str("\n\n");
    md
}

/// Générateur de rapports Markdown.
pub struct ReportGenerator;

impl ReportGenerator {
    /// Génère un rapport Markdown complet à partir d'une analyse.
    pub fn render(report: &AnalysisReport) -> String {
        let mut md = String::new();

        md.push_str(&frontmatter(report));
        md.push_str(&executive_section(report));
        md.push_str(&contributions_section(report));
        md.push_str(&equations_section(report));
        md.push_str(&variables_section(report));
        md.push_str(&algorithms_section(report));
        md.push_str(&system_section(report));
        md.push_str(&risks_section(report));
        md.push_str(&references_section(report));
        md.push_str(&architectural_mapping_section(report));
        md.push_str(&deep_analysis_section(report));
        md.push_str(&experiment_plan_section(report));
        md.push_str(&pseudo_code_section(report));
        md.push_str(&llm_warnings_section(report));
        md.push_str(&recommendation_section(report));

        // Footer
        md.push_str("---\n");
        md.push_str(&format!(
            "*Rapport généré le {} par PAPERS V2 (Rust)*\n",
            report.timestamp
        ));

        md
    }

    /// Sauvegarde un rapport Markdown sur le disque.
    pub fn save(report: &AnalysisReport, path: &Path) -> std::io::Result<()> {
        let md = Self::render(report);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &md)?;
        log::info!("Rapport sauvegardé: {}", path.display());
        Ok(())
    }

    /// Génère un résumé de résultat d'évolution en Markdown.
    pub fn render_evolution_result(
        result: &crate::models::EvolutionResult,
        document_title: &str,
    ) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Résultat d'Évolution: {}\n\n", document_title));
        md.push_str("## Statistiques\n\n");
        md.push_str("| Métrique | Valeur |\n");
        md.push_str("|----------|--------|\n");
        md.push_str(&format!("| Meilleur score | {:.4} |\n", result.best_score));
        md.push_str(&format!("| Rounds | {} |\n", result.total_rounds));
        md.push_str(&format!("| Candidats | {} |\n", result.total_candidates));
        md.push_str(&format!("| Durée | {:.1}s |\n", result.total_time_secs));
        md.push_str(&format!(
            "| Arrêt précoce | {} |\n",
            if result.stopped_early { "oui" } else { "non" }
        ));
        md.push_str(&format!(
            "| Succès | {} |\n",
            if result.success { "oui" } else { "non" }
        ));
        md.push('\n');

        if let Some(ref node) = result.best_node {
            md.push_str("## Meilleur Programme\n\n");
            md.push_str(&format!("**Motivation**: {}\n\n", node.motivation));
            md.push_str("```rust\n");
            md.push_str(&node.code);
            md.push_str("\n```\n\n");

            if !node.results.is_empty() {
                md.push_str("## Résultats d'Évaluation\n\n");
                md.push_str(&format!(
                    "```json\n{}\n```\n",
                    serde_json::to_string_pretty(&node.results).unwrap_or_default()
                ));
            }
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::*;
    use crate::extraction::ExtractedDocument;

    fn make_test_report() -> AnalysisReport {
        AnalysisReport {
            document: ExtractedDocument {
                id: "TEST-001".into(),
                title: "Test Paper".into(),
                authors: vec!["Author One".into()],
                publication_date: Some("2024-01-01".into()),
                source: "arXiv".into(),
                paper_url: Some("https://arxiv.org/abs/2401.00001".into()),
                github_url: Some("https://github.com/test/repo".into()),
                abstract_text: Some("This is a test abstract.".into()),
                full_text: Some("Full text of the paper...".into()),
                references: vec!["Ref 1".into(), "Ref 2".into()],
                parsed: None,
            },
            contributions: vec!["Contribution 1".into()],
            executive_summary: "Executive summary of the paper.".into(),
            equations: vec!["E = mc^2".into()],
            variables: vec![("x".into(), "input".into())],
            algorithms: vec![AlgorithmDesc {
                name: "Test Algo".into(),
                complexity: Some("O(n)".into()),
                pseudocode: Some("for each x: process(x)".into()),
            }],
            system_requirements: SystemRequirements {
                vram: Some("8 GB".into()),
                ram: Some("32 GB".into()),
                disk: None,
                latency: None,
                throughput: None,
                scalability: None,
            },
            risks: vec![Risk {
                level: "LOW".into(),
                description: "Minimal risk".into(),
                mitigation: Some("Monitor".into()),
            }],
            recommendation: Recommendation::Prototype,
            recommendation_justification: "Good integration score.".into(),
            integration_score: 0.65,
            reproducibility_score: 0.5,
            impacted_modules: vec!["memory".into()],
            timestamp: "2024-01-01T00:00:00Z".into(),
            architectural_mapping: serde_json::json!({}),
            deep_analysis: serde_json::Value::Null,
            experiment_plan: serde_json::Value::Null,
            pseudo_code: serde_json::Value::Null,
            llm_warnings: Vec::new(),
        }
    }

    #[test]
    fn test_render_report() {
        let report = make_test_report();
        let md = ReportGenerator::render(&report);

        assert!(md.contains("# Résumé Exécutif"));
        assert!(md.contains("Test Paper"));
        assert!(md.contains("Contribution 1"));
        assert!(md.contains("E = mc^2"));
        assert!(md.contains("PROTOTYPE"));
        assert!(md.contains("PAPERS V2 (Rust)"));
    }

    #[test]
    fn test_save_report() {
        let report = make_test_report();
        let dir = std::env::temp_dir().join("papers_test_reports");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test_report.md");

        ReportGenerator::save(&report, &path).unwrap();
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Test Paper"));

        let _ = fs::remove_file(&path);
    }
}
