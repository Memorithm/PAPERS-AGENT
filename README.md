# PAPERS V2 — Moteur d'évolution autonome en Rust

[![Rust](https://img.shields.io/badge/Rust-10453%20lignes-orange)](papers_core/)
[![Tests](https://img.shields.io/badge/tests-106%20OK-green)](papers_core/)
[![Build](https://img.shields.io/badge/build-0%20warnings-brightgreen)]()
[![Clippy](https://img.shields.io/badge/clippy-0%20warnings-brightgreen)]()

**PAPERS V2** transforme l'analyse de papiers scientifiques en code Rust vivant via une boucle d'évolution autonome.  
Le pipeline complet : **extraction** → **analyse** → **évolution** → **rapport Markdown**.

```
📄 Source (PDF/arXiv/URL) → 🔬 Analyse LLM+heuristique → 🧬 Évolution code Rust → 📊 Rapport .md
```

---

## 🚀 Démarrage rapide

### Prérequis
- **Rust** 1.70+
- **Ollama** (optionnel, pour l'analyse LLM)
- **CUDA** 13.0+ (optionnel, pour l'inférence GPU)
- **GPU NVIDIA** recommandé (Thor/Blackwell ARM64 ou x86_64)

### Build
```bash
cd papers_core
cargo build --release
./target/release/papers --help
```

### Tests
```bash
cargo test          # 106 tests (90 unit + 16 intégration)
```

---

## 📋 CLI — 8 commandes

```bash
# Pipeline complet
papers run -s 2401.00001 --evolve -o ./output

# Extraction seule
papers extract -s paper.pdf -o document.json       # PDF local
papers extract -s 2401.00001 -o arxiv.json         # arXiv
papers extract -s https://example.com              # URL

# Analyse seule
papers analyze -s 2401.00001 --no-llm -o ./output  # Analyse heuristique
papers analyze -s paper.pdf -o ./output            # Avec LLM (gemma4:e2b)

# Évolution seule
papers evolve -t "Implémente un tri parallèle en Rust" -r 50

# Recherche sémantique
papers search -q "reinforcement learning" -k 5

# Génération de rapport
papers report -i analysis.json -o rapport.md

# Statut
papers status
```

### Flags communs
| Flag | Défaut | Description |
|------|--------|-------------|
| `-s, --source` | — | Source (PDF, arXiv ID, URL) |
| `-o, --output` | `./output` | Répertoire de sortie |
| `-r, --rounds` | `50` | Rounds d'évolution max |
| `-p, --policy` | `greedy` | Stratégie d'échantillonnage |
| `-c, --candidates` | `3` | Candidats par round |
| `--model` | `gemma4:e2b` | Modèle LLM |
| `--no-llm` | `false` | Désactiver le LLM (analyse heuristique) |
| `--evolve` | `false` | Activer l'évolution |

---

## 🏗️ Architecture

```
papers_core/                    # Moteur Rust (9969 lignes, 34 modules)
├── src/
│   ├── main.rs                 # Point d'entrée CLI + REPL interactif
│   ├── cli.rs                  # Définition CLI (clap, 10 commandes)
│   ├── config.rs               # Configuration figment (TOML + env vars)
│   ├── engine.rs               # Orchestrateur principal (extract → analyze → evolve)
│   ├── lib.rs                  # Re-exports publics
│   │
│   ├── extraction.rs           # Pipeline PDF/arXiv/URL/Texte
│   ├── paper_parser.rs         # Parser sémantique (regex, sections, équations)
│   ├── paper_registry.rs       # Registre des papiers analysés
│   │
│   ├── doc_store.rs            # Store vectoriel persistant (remplace ChromaDB)
│   ├── vector_store.rs         # HNSW ANN index (remplace FAISS)
│   ├── embedding.rs            # Wrapper scirust-core::EmbeddingEngine (128-dim)
│   ├── embedding_onnx.rs       # ONNX Runtime (all-MiniLM-L6-v2) + fallback déterministe
│   │
│   ├── reporting.rs            # Générateur de rapports Markdown
│   ├── pdf.rs                  # Export PDF structuré (Helvetica/Courier, tables, code)
│   ├── analysis.rs             # Analyse heuristique + scoring
│   ├── llm_analyzer.rs         # Analyse LLM multi-passes (11 méthodes)
│   │
│   ├── evolution/              # Boucle d'évolution Researcher→Engineer→Analyzer
│   │   ├── mod.rs              # EvolutionLoop (run_advanced, UCB1/Greedy/Random)
│   │   ├── researcher.rs       # Génération code Rust par LLM (7 domaines fallback)
│   │   ├── engineer.rs         # Évaluation structurale + WASM sandbox
│   │   └── analyzer.rs         # Analyse résultats + cognition update
│   │
│   ├── llm.rs                  # Client LLM (Ollama + OpenAI)
│   ├── cognition.rs            # Base de connaissances textuelle
│   ├── database.rs             # Base de données avec recherche vectorielle
│   ├── models.rs               # Modèles de données (Node, EvolutionConfig, ...)
│   ├── samplers.rs             # Stratégies d'échantillonnage
│   │
│   ├── symbolic.rs             # Wrapper scirust-symreg + scirust-solvers
│   ├── nas.rs                  # Wrapper scirust-nas
│   ├── graph.rs                # GraphMiner + scirust-neuro-symbolic
│   ├── pattern_induction.rs    # Induction de motifs
│   ├── falsification.rs        # Falsification d'hypothèses
│   ├── verifier.rs             # Vérification formelle
│   ├── probabilistic.rs        # Raisonnement probabiliste
│   │
│   ├── wasm_executor.rs        # Sandbox WASM (wasmtime, fuel limits)
│   ├── gpu.rs                  # Détection CUDA/NVIDIA automatique
│   ├── container.rs            # CacheContainer, KnowledgeContainer, EventContainer
│   ├── queue.rs                # WorkQueue prioritaire, ResultQueue, Backpressure
│   ├── signaling.rs            # EventBus, Signal, Barrier, Latch, Rendezvous
│   └── probes.rs               # Benchmark probe (compilation, perf, sécu)
```

### Dépendances externes
| Crate | Rôle |
|-------|------|
| `scirust-core` | Moteur de calcul scientifique (MiniLLM, autodiff, embedding) |
| `scirust-symreg` | Régression symbolique |
| `scirust-solvers` | Solveurs numériques |
| `scirust-neuro-symbolic` | Graphe de connaissances neuro-symbolique |
| `scirust-nas` | Neural Architecture Search |
| `scirust-symbolic` | Moteur symbolique |
| `wasmtime` | Sandbox WASM pour exécution sécurisée |
| `ort` | ONNX Runtime (inférence on-device) |
| `hnsw_rs` | Index ANN pour recherche vectorielle |
| `figment` | Configuration TOML + env vars |
| `anyhow` / `thiserror` | Gestion d'erreurs |
| `tracing` / `metrics` | Observabilité |
| `pdf-extract` | Extraction texte PDF |
| `reqwest` | Client HTTP (arXiv API, Ollama, OpenAI) |
| `serde` / `clap` / `ndarray` | Sérialisation, CLI, calcul matriciel |

---

## 🧬 Fonctionnement

### Pipeline complet (`papers run`)
1. **Extraction** : le document est extrait (PDF via `pdf-extract`, arXiv via API, URL via HTTP)
2. **Analyse** : le texte est parsé sémantiquement (sections, équations, GitHub URLs, références), puis analysé par le LLM (contributions, résumé exécutif)
3. **Évolution** (si `--evolve`) : une boucle d'évolution génère du code Rust via le LLM, évalue les candidats, et converge vers la meilleure solution
4. **Rapport** : un rapport Markdown complet est généré (`analysis_report.md`, `evolution_report.md`, `best_program.rs`)

### Évolution
- **Config** : rounds, candidats/round, politique d'échantillonnage, patience
- **Samplers** : `greedy`, `ucb1`, `random`, `island`
- **Évaluation** : le LLM génère du code Rust, un score heuristique est attribué (présence de `fn`, `struct`, accolades)
- **Arrêt** : score cible atteint OU patience (rounds sans amélioration)

### Embedding
- **Moteur** : `scirust-core::EmbeddingEngine` — MiniLLM char-level transformer 128-dim
- **Utilisation** : recherche sémantique de papiers, indexation automatique des nœuds d'évolution
- **Remplace** : `sentence-transformers/all-MiniLM-L6-v2` (384-dim), `chromadb`, `faiss-cpu`

---

## 📊 Migration Python → Rust

| Module Python | Module Rust | Lignes | Statut |
|---------------|-------------|--------|--------|
| `extraction/extractors.py` | `extraction.rs` | 421 | ✅ Fini |
| `extraction/semantic.py` | `paper_parser.rs` | 283 | ✅ Fini |
| `knowledge/vector_store.py` | `doc_store.rs` | 368 | ✅ Fini |
| `core/orchestrator.py` | `engine.rs` | 410 | ✅ Fini |
| `utils/reporting.py` | `reporting.rs` | 263 | ✅ Fini |
| `infra/llm_client.py` | `llm.rs` | 137 | ✅ Fini |
| `infra/faiss_index.py` | `vector_store.rs` | 265 | ✅ Fini |
| `infra/embedding.py` | `embedding.rs` | 132 | ✅ Fini |
| `evolution/*` | `evolution.rs` + `cognition.rs` + `database.rs` | 373 | 🔄 Partiel |
| `intelligence/*` | 6 modules Rust | 648 | ✅ Fini |

---

## 🔧 Modèles LLM

Ollama doit être en cours d'exécution (`ollama serve`). Modèles testés :

| Modèle | Taille | Usage |
|--------|--------|-------|
| `gemma4:e2b` | 7.2 Go | Rapide (~68 tok/s), par défaut |
| `qwen3:4b` | 2.5 Go | Ultra-rapide |
| `gemma4:31b` | 19 Go | Haute qualité |
| `qwen3-coder:30b` | 18 Go | Optimisé code |

Détection GPU automatique. Sur NVIDIA Thor (ARM64) → `ollama ps` affiche `100% GPU`.

---

## 📝 Exemple de sortie

```bash
$ papers run -s 2401.00001 --evolve -o /tmp/example

╔══════════════════════════════════════════╗
║   PAPERS V2 - Pipeline Complet (Rust)    ║
╚══════════════════════════════════════════╝

📄 Source: 2401.00001
🤖 LLM: gemma4:e2b
🧬 Évolution: activée
📁 Sortie: /tmp/example

✅ Extraction: Sector Rotation Strategy (arXiv)
✅ Analyse: score intégration = 0.65, recommandation = PROTOTYPE
🧬 Évolution: best=0.7234, 150 candidats, 45.2s

📊 Rapports sauvegardés dans: /tmp/example
   - analysis_report.md
   - extracted_document.json
   - evolution_report.md
   - best_program.rs

⏱️  Durée totale: 47.3s
```

---

## 📄 Licence

MIT — CHECKUPAUTO 2024-2026
