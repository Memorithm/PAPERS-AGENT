use std::fs;
use std::path::Path;

use papers_core::cli::{parse, Commands};
use papers_core::cognition::CognitionStore;
use papers_core::doc_store::DocStore;
use papers_core::engine::PapersEngine;
use papers_core::extraction::ExtractionPipeline;
use papers_core::llm::LlmConfig;
use papers_core::models::{CognitionItem, EvolutionConfig};
use papers_core::reporting::ReportGenerator;

fn main() {
    env_logger::init();
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
                ..Default::default()
            };

            let mut engine = PapersEngine::new(use_llm, Some(llm_config));

            println!("📄 Source: {}", source);
            println!("🤖 LLM: {}", if use_llm { &model } else { "désactivé" });
            println!("🧬 Évolution: {}", if evolve { "activée" } else { "désactivée" });
            println!("📁 Sortie: {}", output);
            println!();

            // Créer le répertoire de sortie
            let output_dir = Path::new(&output);
            fs::create_dir_all(output_dir).unwrap_or_else(|e| {
                eprintln!("Erreur création répertoire: {}", e);
            });

            let evo_config = if evolve {
                Some(EvolutionConfig {
                    task_description: format!("Implémente en Rust les concepts extraits de: {}", source),
                    max_rounds: rounds,
                    sampling_policy: policy,
                    n_candidates_per_round: candidates,
                    ..Default::default()
                })
            } else {
                None
            };

            match engine.run_pipeline(&source, evolve, evo_config) {
                Ok(result) => {
                    println!("✅ Extraction: {} ({})", result.document.title, result.document.source);
                    println!("✅ Analyse: score intégration = {:.2}, recommandation = {}",
                        result.analysis.integration_score,
                        result.analysis.recommendation.label(),
                    );

                    // Sauvegarder le rapport d'analyse
                    let report_path = output_dir.join("analysis_report.md");
                    ReportGenerator::save(&result.analysis, &report_path).unwrap_or_else(|e| {
                        eprintln!("Erreur sauvegarde rapport: {}", e);
                    });

                    // Sauvegarder le document extrait en JSON
                    let doc_path = output_dir.join("extracted_document.json");
                    let doc_json = serde_json::to_string_pretty(&result.document).unwrap_or_default();
                    fs::write(&doc_path, doc_json).unwrap_or_else(|e| {
                        eprintln!("Erreur sauvegarde document: {}", e);
                    });

                    if let Some(ref evo_result) = result.evolution {
                        println!("🧬 Évolution: best={:.4}, {} candidats, {:.1}s",
                            evo_result.best_score,
                            evo_result.total_candidates,
                            evo_result.total_time_secs,
                        );

                        // Sauvegarder le rapport d'évolution
                        let evo_path = output_dir.join("evolution_report.md");
                        let evo_md = ReportGenerator::render_evolution_result(
                            evo_result,
                            &result.document.title,
                        );
                        fs::write(&evo_path, evo_md).unwrap_or_else(|e| {
                            eprintln!("Erreur sauvegarde évolution: {}", e);
                        });

                        // Sauvegarder le meilleur code
                        if let Some(ref node) = evo_result.best_node {
                            let code_path = output_dir.join("best_program.rs");
                            fs::write(&code_path, &node.code).unwrap_or_else(|e| {
                                eprintln!("Erreur sauvegarde code: {}", e);
                            });
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
                Err(e) => {
                    eprintln!("❌ Erreur pipeline: {}", e);
                    std::process::exit(1);
                }
            }
        }

        // ── Extraction seule ─────────────────────────────────
        Commands::Extract { source, output } => {
            println!("📄 Extraction: {}", source);
            let pipeline = ExtractionPipeline::new();

            match pipeline.extract(&source) {
                Ok(doc) => {
                    let json = serde_json::to_string_pretty(&doc).unwrap_or_default();

                    if let Some(ref path) = output {
                        fs::write(path, &json).unwrap_or_else(|e| {
                            eprintln!("Erreur écriture: {}", e);
                        });
                        println!("✅ Document sauvegardé: {}", path);
                    } else {
                        println!("{}", json);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Erreur extraction: {}", e);
                    std::process::exit(1);
                }
            }
        }

        // ── Analyse seule ────────────────────────────────────
        Commands::Analyze { source, no_llm, model, output } => {
            println!("🔬 Analyse: {}", source);

            let use_llm = !no_llm;
            let llm_config = LlmConfig { model: model.clone(), ..Default::default() };
            let mut engine = PapersEngine::new(use_llm, Some(llm_config));

            match engine.extract(&source) {
                Ok(doc) => {
                    let analysis = engine.analyze(&doc);

                    let output_dir = Path::new(&output);
                    fs::create_dir_all(output_dir).unwrap_or_else(|e| {
                        eprintln!("Erreur création répertoire: {}", e);
                    });

                    let report_path = output_dir.join("analysis_report.md");
                    ReportGenerator::save(&analysis, &report_path).unwrap_or_else(|e| {
                        eprintln!("Erreur sauvegarde: {}", e);
                    });

                    let json_path = output_dir.join("analysis.json");
                    let json = serde_json::to_string_pretty(&analysis).unwrap_or_default();
                    fs::write(&json_path, json).unwrap_or_else(|e| {
                        eprintln!("Erreur JSON: {}", e);
                    });

                    println!("✅ Recommandation: {}", analysis.recommendation.label());
                    println!("✅ Score intégration: {:.2}", analysis.integration_score);
                    println!("📊 Rapport: {}", report_path.display());
                }
                Err(e) => {
                    eprintln!("❌ Erreur: {}", e);
                    std::process::exit(1);
                }
            }
        }

        // ── Évolution seule ──────────────────────────────────
        Commands::Evolve { task, source, rounds, policy, candidates, model, output } => {
            println!("🧬 PAPER EVOLVE");
            println!("Tâche: {}", task);
            println!("Modèle: {}", model);

            let llm_config = LlmConfig { model: model.clone(), ..Default::default() };
            let mut engine = PapersEngine::new(true, Some(llm_config));

            let config = EvolutionConfig {
                task_description: task.clone(),
                max_rounds: rounds,
                sampling_policy: policy,
                n_candidates_per_round: candidates,
                ..Default::default()
            };

            // Si une source de papier est fournie, l'extraire d'abord
            let doc = if let Some(ref src) = source {
                match engine.extract(src) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("⚠️  Impossible d'extraire la source: {}", e);
                        eprintln!("   L'évolution continuera sans contexte de papier.");
                        return;
                    }
                }
            } else {
                // Document factice pour l'évolution sans source
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

            match engine.evolve(&doc, config) {
                Ok(result) => {
                    println!();
                    println!("═══════════════════════════════════════");
                    println!("  Évolution terminée");
                    println!("═══════════════════════════════════════");
                    println!("  Meilleur score: {:.4}", result.best_score);
                    println!("  Rounds: {}", result.total_rounds);
                    println!("  Candidats: {}", result.total_candidates);
                    println!("  Durée: {:.1}s", result.total_time_secs);
                    println!("  Arrêt précoce: {}", if result.stopped_early { "oui" } else { "non" });

                    let output_dir = Path::new(&output);
                    fs::create_dir_all(output_dir).unwrap_or_else(|e| {
                        eprintln!("Erreur répertoire: {}", e);
                    });

                    // Rapport
                    let report_md = ReportGenerator::render_evolution_result(&result, &doc.title);
                    let report_path = output_dir.join("evolution_report.md");
                    fs::write(&report_path, &report_md).unwrap_or_else(|e| {
                        eprintln!("Erreur rapport: {}", e);
                    });

                    // Meilleur code
                    if let Some(ref node) = result.best_node {
                        let code_path = output_dir.join("best_program.rs");
                        fs::write(&code_path, &node.code).unwrap_or_else(|e| {
                            eprintln!("Erreur code: {}", e);
                        });
                        println!("  Code: {}", code_path.display());
                    }

                    println!("  Rapport: {}", report_path.display());
                }
                Err(e) => {
                    eprintln!("❌ Erreur évolution: {}", e);
                    std::process::exit(1);
                }
            }
        }

        // ── Recherche sémantique ─────────────────────────────
        Commands::Search { query, top_k } => {
            let corpus = &["papers research AI machine learning"];
            let mut store = DocStore::new(corpus);

            // Charger les documents existants si la base existe
            let db_path = Path::new("./doc_store.json");
            if db_path.exists() {
                match DocStore::with_persistence(corpus, db_path) {
                    Ok(s) => store = s,
                    Err(e) => eprintln!("⚠️  Impossible de charger la base: {}", e),
                }
            }

            println!("🔍 Recherche: \"{}\"", query);
            let results = store.search(&query, top_k);

            if results.is_empty() {
                println!("Aucun résultat.");
            } else {
                for (i, r) in results.iter().enumerate() {
                    println!("{}. [{}] {} (sim: {:.3})",
                        i + 1, r.id, r.text.chars().take(80).collect::<String>(),
                        r.similarity);
                }
            }
        }

        // ── Génération de rapport ────────────────────────────
        Commands::Report { input, output } => {
            match fs::read_to_string(&input) {
                Ok(json) => {
                    match serde_json::from_str::<papers_core::engine::AnalysisReport>(&json) {
                        Ok(report) => {
                            let md = ReportGenerator::render(&report);
                            if let Some(ref path) = output {
                                fs::write(path, &md).unwrap_or_else(|e| {
                                    eprintln!("Erreur écriture: {}", e);
                                });
                                println!("✅ Rapport: {}", path);
                            } else {
                                println!("{}", md);
                            }
                        }
                        Err(e) => eprintln!("❌ JSON invalide: {}", e),
                    }
                }
                Err(e) => eprintln!("❌ Impossible de lire {}: {}", input, e),
            }
        }

        // ── Statut ───────────────────────────────────────────
        Commands::Status { db } => {
            println!("📊 Statut PAPERS V2");
            println!("Fichier DB: {}", db);

            let db_path = Path::new(&db);
            if db_path.exists() {
                let content = fs::read_to_string(db_path).unwrap_or_default();
                let nodes: Vec<papers_core::models::Node> =
                    serde_json::from_str(&content).unwrap_or_default();
                println!("  Nœuds: {}", nodes.len());
                if !nodes.is_empty() {
                    let avg_score: f64 = nodes.iter().map(|n| n.score).sum::<f64>() / nodes.len() as f64;
                    let max_score = nodes.iter().map(|n| n.score).fold(f64::NEG_INFINITY, f64::max);
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

            let content = fs::read_to_string(&file).unwrap_or_else(|e| {
                eprintln!("Erreur lecture: {}", e);
                std::process::exit(1);
            });

            // Essayer de parser comme JSON (array d'items)
            if let Ok(items) = serde_json::from_str::<Vec<CognitionItem>>(&content) {
                store.add_batch(items);
            } else {
                // Fallback: traiter comme texte brut
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

            // Sauvegarder
            let seed_path = Path::new(&file).with_extension("seeded.json");
            let seed_json = serde_json::to_string_pretty(
                &store.retrieve("", store.len()).iter().map(|i| (*i).clone()).collect::<Vec<_>>()
            ).unwrap_or_default();
            fs::write(&seed_path, seed_json).unwrap_or_else(|e| {
                eprintln!("Erreur sauvegarde: {}", e);
            });

            println!("✅ {} items ajoutés → {}", store.len(), seed_path.display());
        }
    }
}
