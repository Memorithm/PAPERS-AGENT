use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use papers_core::cli::{parse, Commands};
use papers_core::cognition::CognitionStore;
use papers_core::config::PapersConfig;
use papers_core::doc_store::DocStore;
use papers_core::engine::PapersEngine;
use papers_core::extraction::ExtractionPipeline;
use papers_core::llm::LlmConfig;
use papers_core::models::{CognitionItem, EvolutionConfig};
use papers_core::reporting::ReportGenerator;

fn main() -> Result<()> {
    env_logger::init();

    // Load configuration
    let config = PapersConfig::load(None).context("Failed to load configuration")?;

    let cli = parse();

    match cli.command {
        // ── Pipeline complet ──────────────────────────────────
        Commands::Run {
            source,
            evolve,
            no_llm,
            model,
            output,
            rounds,
            policy,
            candidates,
        } => {
            println!("╔══════════════════════════════════════════╗");
            println!("║   PAPERS V2 - Pipeline Complet (Rust)    ║");
            println!("╚══════════════════════════════════════════╝");
            println!();

            let use_llm = !no_llm;
            let llm_config = LlmConfig {
                model: model.clone(),
                ..config.to_llm_config()
            };

            let mut engine = PapersEngine::new(use_llm, Some(llm_config));

            println!("📄 Source: {}", source);
            println!("🤖 LLM: {}", if use_llm { &model } else { "désactivé" });
            println!(
                "🧬 Évolution: {}",
                if evolve { "activée" } else { "désactivée" }
            );
            println!("📁 Sortie: {}", output);
            println!();

            let output_dir = Path::new(&output);
            fs::create_dir_all(output_dir).context("Failed to create output directory")?;

            let evo_config = if evolve {
                Some(EvolutionConfig {
                    task_description: format!(
                        "Implémente en Rust les concepts extraits de: {}",
                        source
                    ),
                    max_rounds: rounds,
                    sampling_policy: policy,
                    n_candidates_per_round: candidates,
                    ..config.to_evolution_config()
                })
            } else {
                None
            };

            let result = engine
                .run_pipeline(&source, evolve, evo_config)
                .map_err(|e| anyhow::anyhow!(e))?;

            println!(
                "✅ Extraction: {} ({})",
                result.document.title, result.document.source
            );
            println!(
                "✅ Analyse: score intégration = {:.2}, recommandation = {}",
                result.analysis.integration_score,
                result.analysis.recommendation.label(),
            );

            let report_path = output_dir.join("analysis_report.md");
            ReportGenerator::save(&result.analysis, &report_path)
                .context("Failed to save analysis report")?;

            let doc_path = output_dir.join("extracted_document.json");
            let doc_json = serde_json::to_string_pretty(&result.document)
                .context("Failed to serialize document")?;
            fs::write(&doc_path, doc_json).context("Failed to save document")?;

            if let Some(ref evo_result) = result.evolution {
                println!(
                    "🧬 Évolution: best={:.4}, {} candidats, {:.1}s",
                    evo_result.best_score, evo_result.total_candidates, evo_result.total_time_secs,
                );

                let evo_path = output_dir.join("evolution_report.md");
                let evo_md =
                    ReportGenerator::render_evolution_result(evo_result, &result.document.title);
                fs::write(&evo_path, evo_md).context("Failed to save evolution report")?;

                if let Some(ref node) = evo_result.best_node {
                    let code_path = output_dir.join("best_program.rs");
                    fs::write(&code_path, &node.code).context("Failed to save best program")?;
                    println!("📝 Meilleur code: {}", code_path.display());
                }
            }

            println!();
            println!("📊 Rapports sauvegardés dans: {}", output_dir.display());
            println!("   - analysis_report.md");
            println!("   - extracted_document.json");
            if evolve {
                println!("   - evolution_report.md");
                println!("   - best_program.rs");
            }
            println!();
            println!("⏱️  Durée totale: {:.1}s", result.duration_secs);
        }

        // ── Extraction seule ─────────────────────────────────
        Commands::Extract { source, output } => {
            println!("📄 Extraction: {}", source);
            let pipeline = ExtractionPipeline::new();

            let doc = pipeline.extract(&source).map_err(|e| anyhow::anyhow!(e))?;
            let json =
                serde_json::to_string_pretty(&doc).context("Failed to serialize document")?;

            if let Some(ref path) = output {
                fs::write(path, &json).context("Failed to write document")?;
                println!("✅ Document sauvegardé: {}", path);
            } else {
                println!("{}", json);
            }
        }

        // ── Analyse seule ────────────────────────────────────
        Commands::Analyze {
            source,
            no_llm,
            model,
            output,
        } => {
            println!("🔬 Analyse: {}", source);

            let use_llm = !no_llm;
            let llm_config = LlmConfig {
                model: model.clone(),
                ..config.to_llm_config()
            };
            let mut engine = PapersEngine::new(use_llm, Some(llm_config));

            let doc = engine.extract(&source).map_err(|e| anyhow::anyhow!(e))?;
            let analysis = engine.analyze(&doc);

            let output_dir = Path::new(&output);
            fs::create_dir_all(output_dir).context("Failed to create output directory")?;

            let report_path = output_dir.join("analysis_report.md");
            ReportGenerator::save(&analysis, &report_path).context("Failed to save report")?;

            let json_path = output_dir.join("analysis.json");
            let json =
                serde_json::to_string_pretty(&analysis).context("Failed to serialize analysis")?;
            fs::write(&json_path, json).context("Failed to save JSON")?;

            println!("✅ Recommandation: {}", analysis.recommendation.label());
            println!("✅ Score intégration: {:.2}", analysis.integration_score);
            println!("📊 Rapport: {}", report_path.display());
        }

        // ── Évolution seule ──────────────────────────────────
        Commands::Evolve {
            task,
            source,
            rounds,
            policy,
            candidates,
            model,
            output,
        } => {
            println!("🧬 PAPER EVOLVE");
            println!("Tâche: {}", task);
            println!("Modèle: {}", model);

            let llm_config = LlmConfig {
                model: model.clone(),
                ..config.to_llm_config()
            };
            let mut engine = PapersEngine::new(true, Some(llm_config));

            let evo_config = EvolutionConfig {
                task_description: task.clone(),
                max_rounds: rounds,
                sampling_policy: policy,
                n_candidates_per_round: candidates,
                ..config.to_evolution_config()
            };

            let doc = if let Some(ref src) = source {
                match engine.extract(src) {
                    Ok(d) => d,
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Extraction failed: {}. Cannot evolve without source document.",
                            e
                        ));
                    }
                }
            } else {
                use papers_core::extraction::ExtractedDocument;
                ExtractedDocument {
                    id: "EVOLVE-001".into(),
                    title: "Tâche d'évolution".into(),
                    authors: vec![],
                    publication_date: None,
                    source: "CLI".into(),
                    paper_url: None,
                    github_url: None,
                    abstract_text: Some(task.clone()),
                    full_text: Some(task.clone()),
                    references: vec![],
                    parsed: None,
                }
            };

            let result = engine
                .evolve(&doc, evo_config)
                .map_err(|e| anyhow::anyhow!(e))?;

            println!();
            println!("═══════════════════════════════════════");
            println!("  Évolution terminée");
            println!("═══════════════════════════════════════");
            println!("  Meilleur score: {:.4}", result.best_score);
            println!("  Rounds: {}", result.total_rounds);
            println!("  Candidats: {}", result.total_candidates);
            println!("  Durée: {:.1}s", result.total_time_secs);
            println!(
                "  Arrêt précoce: {}",
                if result.stopped_early { "oui" } else { "non" }
            );

            let output_dir = Path::new(&output);
            fs::create_dir_all(output_dir).context("Failed to create output directory")?;

            let report_md = ReportGenerator::render_evolution_result(&result, &doc.title);
            let report_path = output_dir.join("evolution_report.md");
            fs::write(&report_path, &report_md).context("Failed to save evolution report")?;

            if let Some(ref node) = result.best_node {
                let code_path = output_dir.join("best_program.rs");
                fs::write(&code_path, &node.code).context("Failed to save best program")?;
                println!("  Code: {}", code_path.display());
            }

            println!("  Rapport: {}", report_path.display());
        }

        // ── Recherche sémantique ─────────────────────────────
        Commands::Search { query, top_k } => {
            let db_path = Path::new("./doc_store.json");
            let corpus = &["papers research AI machine learning"];
            let mut store = if db_path.exists() {
                DocStore::with_persistence(corpus, db_path).unwrap_or_else(|e| {
                    eprintln!("⚠️  Impossible de charger la base: {}", e);
                    DocStore::new(corpus)
                })
            } else {
                DocStore::new(corpus)
            };

            println!("🔍 Recherche: \"{}\"", query);
            let results = store.search(&query, top_k);

            if results.is_empty() {
                println!("Aucun résultat.");
            } else {
                for (i, r) in results.iter().enumerate() {
                    println!(
                        "{}. [{}] {} (sim: {:.3})",
                        i + 1,
                        r.id,
                        r.text.chars().take(80).collect::<String>(),
                        r.similarity
                    );
                }
            }
        }

        // ── Génération de rapport ────────────────────────────
        Commands::Report { input, output } => {
            let json = fs::read_to_string(&input).context(format!("Cannot read {}", input))?;
            let report: papers_core::engine::AnalysisReport =
                serde_json::from_str(&json).context("Invalid JSON")?;
            let md = ReportGenerator::render(&report);
            if let Some(ref path) = output {
                fs::write(path, &md).context("Failed to write report")?;
                println!("✅ Rapport: {}", path);
            } else {
                println!("{}", md);
            }
        }

        // ── Statut ───────────────────────────────────────────
        Commands::Status { db } => {
            println!("📊 Statut PAPERS V2");
            println!("Fichier DB: {}", db);

            let db_path = Path::new(&db);
            if db_path.exists() {
                let content = fs::read_to_string(db_path).context("Failed to read database")?;
                let nodes: Vec<papers_core::models::Node> =
                    serde_json::from_str(&content).context("Failed to parse database")?;
                println!("  Nœuds: {}", nodes.len());
                if !nodes.is_empty() {
                    let avg_score: f64 =
                        nodes.iter().map(|n| n.score).sum::<f64>() / nodes.len() as f64;
                    let max_score = nodes
                        .iter()
                        .map(|n| n.score)
                        .fold(f64::NEG_INFINITY, f64::max);
                    println!("  Score moyen: {:.4}", avg_score);
                    println!("  Meilleur score: {:.4}", max_score);
                }
            } else {
                println!("  Base de données non trouvée.");
            }
        }

        // ── Seed cognition ───────────────────────────────────
        Commands::Seed { file } => {
            println!("🌱 Peuplement cognition: {}", file);
            let mut store = CognitionStore::new();

            let content = fs::read_to_string(&file).context(format!("Cannot read {}", file))?;

            if let Ok(items) = serde_json::from_str::<Vec<CognitionItem>>(&content) {
                store.add_batch(items);
            } else {
                for (i, paragraph) in content.split("\n\n").enumerate() {
                    let trimmed = paragraph.trim();
                    if !trimmed.is_empty() {
                        store.add(CognitionItem::new(
                            trimmed.to_string(),
                            file.clone(),
                            vec![format!("paragraph-{}", i)],
                        ));
                    }
                }
            }

            let seed_path = Path::new(&file).with_extension("seeded.json");
            let seed_json = serde_json::to_string_pretty(
                &store
                    .retrieve_all()
                    .iter()
                    .map(|i| (*i).clone())
                    .collect::<Vec<_>>(),
            )
            .context("Failed to serialize")?;
            fs::write(&seed_path, seed_json).context("Failed to save seeded data")?;

            println!("✅ {} items ajoutés → {}", store.len(), seed_path.display());
        }

        // ── Mode interactif REPL ─────────────────────────────
        Commands::Interactive { model } => {
            run_interactive(&config, &model)?;
        }

        // ── Export PDF ───────────────────────────────────────
        Commands::Pdf { input, output } => {
            let json = fs::read_to_string(&input).context(format!("Cannot read {}", input))?;
            let report: papers_core::engine::AnalysisReport =
                serde_json::from_str(&json).context("Invalid JSON")?;
            let md = ReportGenerator::render(&report);

            let output_path = output.map(PathBuf::from).unwrap_or_else(|| {
                let mut p = PathBuf::from(&input);
                p.set_extension("pdf");
                p
            });

            let pdf_bytes = generate_pdf(&report, &md)?;
            fs::write(&output_path, pdf_bytes).context("Failed to write PDF")?;
            println!("✅ PDF: {}", output_path.display());
        }
    }

    Ok(())
}

/// Interactive REPL mode.
fn run_interactive(config: &PapersConfig, model: &str) -> Result<()> {
    use std::io::{self, BufRead, Write};

    let llm_config = LlmConfig {
        model: model.to_string(),
        ..config.to_llm_config()
    };
    let mut engine = PapersEngine::new(true, Some(llm_config));

    println!("╔══════════════════════════════════════════╗");
    println!("║   PAPERS V2 - Mode Interactif (REPL)     ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ Commandes: extract, analyze, search,     ║");
    println!("║           status, config, help, exit     ║");
    println!("╚══════════════════════════════════════════╝");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("papers> ");
        stdout.flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[0] {
            "exit" | "quit" => {
                println!("Au revoir.");
                break;
            }
            "help" => {
                println!("Commandes:");
                println!("  extract <source>  - Extraire un papier");
                println!("  analyze <source>  - Analyser un papier");
                println!("  search <query>    - Recherche sémantique");
                println!("  status            - Statut du système");
                println!("  config            - Configuration");
                println!("  help              - Cette aide");
                println!("  exit|quit         - Quitter");
            }
            "extract" => {
                let source = parts[1..].join(" ");
                if source.is_empty() {
                    eprintln!("Usage: extract <source>");
                    continue;
                }
                match engine.extract(&source).map_err(|e| anyhow::anyhow!(e)) {
                    Ok(doc) => println!("✅ {} (auteurs: {})", doc.title, doc.authors.join(", ")),
                    Err(e) => eprintln!("❌ {}", e),
                }
            }
            "analyze" => {
                let source = parts[1..].join(" ");
                if source.is_empty() {
                    eprintln!("Usage: analyze <source>");
                    continue;
                }
                match engine.extract(&source) {
                    Ok(doc) => {
                        let a = engine.analyze(&doc);
                        println!(
                            "📊 Intégration: {:.2} | Recommandation: {}",
                            a.integration_score,
                            a.recommendation.label()
                        );
                    }
                    Err(e) => eprintln!("❌ {}", e),
                }
            }
            "search" => {
                let query = parts[1..].join(" ");
                if query.is_empty() {
                    eprintln!("Usage: search <query>");
                    continue;
                }
                let results = engine.find_similar(&query, 5);
                if results.is_empty() {
                    println!("Aucun résultat.");
                } else {
                    for r in &results {
                        println!(
                            "  [{}] {} (sim: {:.3})",
                            r.id,
                            r.text.chars().take(80).collect::<String>(),
                            r.similarity
                        );
                    }
                }
            }
            "status" => {
                println!("📊 PAPERS V2");
                println!("   Embedding: ONNX (fallback déterministe)");
                println!("   Index: HNSW (ANN)");
                println!("   Documents: {}", engine.doc_store.len());
            }
            "config" => {
                println!("📋 LLM: {} @ {}", config.llm.model, config.llm.base_url);
                println!(
                    "   Évolution: {} rounds, {}",
                    config.evolution.max_rounds, config.evolution.sampling_policy
                );
                println!("   Embedding: {} dim", config.embedding.dim);
            }
            other => eprintln!("Commande inconnue: '{}'. Tapez 'help'.", other),
        }
    }
    Ok(())
}

/// Simple text-based PDF generation (using enhanced PdfDocument).
fn generate_pdf(report: &papers_core::engine::AnalysisReport, _md: &str) -> Result<Vec<u8>> {
    use papers_core::pdf::PdfDocument;
    let doc = &report.document;

    let algorithms: Vec<(&str, Option<&str>)> = report
        .algorithms
        .iter()
        .map(|a| (a.name.as_str(), a.complexity.as_deref()))
        .collect();

    PdfDocument::from_analysis_report(
        &doc.title,
        &doc.authors,
        &doc.source,
        report.integration_score,
        report.recommendation.label(),
        &report.executive_summary,
        &report.contributions,
        &report.equations,
        &algorithms,
    )
    .context("Failed to generate PDF report")
}
