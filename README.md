# PAPERS V2 — Moteur d'évolution autonome en Rust

[![Rust](https://img.shields.io/badge/Rust-15%2C7k%20lignes-orange)](papers_core/)
[![Tests](https://img.shields.io/badge/tests-195%20OK-green)](papers_core/)
[![Build](https://img.shields.io/badge/build-0%20warnings-brightgreen)]()
[![Clippy](https://img.shields.io/badge/clippy--strict-0%20warnings-brightgreen)]()
[![Audit](https://img.shields.io/badge/cargo--audit-0%20vuln%C3%A9rabilit%C3%A9-brightgreen)]()

**PAPERS V2** transforme l'analyse de papiers scientifiques en hypothèses vérifiables et en code Rust candidat via une boucle d'évolution.  
Le pipeline complet : **extraction** → **analyse** → **évolution** → **rapport** (Markdown/PDF) → **bundle scientifique versionné**.

```
📄 Source (PDF/arXiv/URL) → 🔬 Analyse LLM+heuristique → 🧬 Évolution code Rust
                          → 📊 Rapport .md/.pdf → 📦 bundle memorithm.science/bundle-v1
                          → 🧪 soumission CCOS Research Lab (exécution empirique)
```

> **Principe de responsabilité** : PAPERS *propose* des hypothèses et des candidats de code.
> L'évaluation empirique appartient à un runtime externe (CCOS Research Lab / RSI) — le core
> refuse par conception de fabriquer des scores synthétiques (`nas.rs` fail-closed,
> `wasm_executor` n'exécute que du WASM réellement compilé).

---

## 🚀 Démarrage rapide

### Prérequis
- **Rust** 1.75+
- **Ollama** (optionnel, pour l'analyse LLM) ou toute API OpenAI-compatible
- `rustup target add wasm32-unknown-unknown` (optionnel, pour exécuter les candidats en sandbox)

### Build
```bash
cd papers_core
cargo build --release
./target/release/papers --help
```

### Tests
```bash
cargo test          # 195 tests (173 unit + 22 intégration), clippy strict -D warnings
cargo audit         # 0 vulnérabilité connue (CI bloquante)
```

---

## 📋 CLI — 12 commandes

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

# Recherche sémantique (indexe le corpus seed si aucune base n'existe)
papers search -q "reinforcement learning" -k 5

# Génération de rapport / PDF
papers report -i analysis.json -o rapport.md
papers pdf -i analysis.json

# Surveillance arXiv : nouveaux papiers par topic dans le registre
papers watch --topics "recursive self-improvement" "agent memory" \
             --interval-secs 3600 --registry ./papers_registry.json

# Métriques Prometheus
papers serve-metrics --port 9091                   # GET /metrics

# Mode interactif (REPL)
papers interactive
```

### Export scientific bundle (`papers-contract`)
```bash
# JSON (défaut), JSONL (streaming) ou CSV (table plate des claims)
papers-contract -i analysis.json --format jsonl
papers-contract -i analysis.json --format csv

# Soumission au CCOS Research Lab + polling optionnel
papers-contract -i analysis.json --lab-url http://127.0.0.1:8080 \
                [--lab-key sk-...] [--wait-secs 600]
```

### Flags communs
| Flag | Défaut | Description |
|------|--------|-------------|
| `-s, --source` | — | Source (PDF, arXiv ID, URL) |
| `-o, --output` | `./output` | Répertoire de sortie |
| `-r, --rounds` | `50` | Rounds d'évolution max |
| `-p, --policy` | `greedy` | Stratégie d'échantillonnage (`greedy`, `ucb1`, `random`, `island`) |
| `-c, --candidates` | `3` | Candidats par round |
| `--model` | `gemma4:e2b` | Modèle LLM (source unique : `config.rs`) |
| `--no-llm` | `false` | Désactiver le LLM (analyse heuristique) |

---

## 🏗️ Architecture

```
papers_core/                    # ~15,7k lignes, 44 fichiers
├── src/
│   ├── main.rs                 # Point d'entrée CLI + REPL + watch + serve-metrics
│   ├── cli.rs                  # Définition CLI (clap, 12 commandes)
│   ├── config.rs               # Configuration figment (TOML + env vars)
│   ├── engine.rs               # Orchestrateur principal + corpus seed partagé
│   ├── lib.rs                  # Re-exports publics
│   │
│   ├── extraction.rs           # Pipeline PDF/arXiv/URL/Texte + recherche arXiv (watch)
│   ├── paper_parser.rs         # Parser sémantique (regex, sections, équations)
│   ├── paper_registry.rs       # Registre persistant : backup .bak, écriture atomique,
│   │                           #   refus d'écraser un fichier corrompu
│   │
│   ├── doc_store.rs            # Store vectoriel persistant
│   ├── vector_store.rs         # HNSW ANN index
│   ├── embedding.rs            # Embedding TF-IDF + feature hashing signé (128-dim),
│   │                           #   déterministe et rejouable
│   ├── embedding_onnx.rs       # ONNX Runtime MiniLM + fallback déterministe
│   ├── embedding_cache.rs      # Cache disque persistant (clé SHA-256 dim|texte)
│   │
│   ├── reporting.rs            # Rapports Markdown (+ section avertissements LLM)
│   ├── pdf.rs                  # Export PDF structuré (Helvetica/Courier, tables, code)
│   ├── analysis.rs             # Analyse heuristique + scoring normalisé [0,1]
│   ├── llm_analyzer.rs         # Analyse LLM multi-passes ; mode strict collecte les
│   │                           #   échecs explicitement (llm_warnings du rapport)
│   │
│   ├── evolution/              # Boucle Researcher→Engineer→Analyzer
│   │   ├── mod.rs              # EvolutionLoop (run_advanced, UCB1/Greedy/Random/Island)
│   │   ├── researcher.rs       # Génération code Rust par LLM + fallback templates
│   │   ├── engineer.rs         # Évaluation structurelle + sandbox WASM
│   │   └── analyzer.rs         # Analyse résultats + cognition update
│   │
│   ├── llm.rs                  # Client Ollama + OpenAI-compatible : timeouts, retry
│   │                           #   exponentiel sur 5xx/réseau, api_key optionnelle
│   ├── ccos.rs                 # Client CCOS Research Lab : submit/poll/result/wait
│   ├── metrics_server.rs       # Compteurs Prometheus + serveur HTTP /metrics
│   ├── cognition.rs            # Base de connaissances textuelle
│   ├── database.rs             # Base de nœuds + recherche vectorielle optionnelle
│   ├── models.rs               # Node, EvolutionConfig, ...
│   ├── samplers.rs             # greedy / ucb1 / random / island
│   │
│   ├── symbolic.rs             # Régression affine moindres carrés, RK4, Simpson
│   ├── nas.rs                  # Fail-closed : délègue le NAS au runtime externe
│   ├── graph.rs                # GraphMiner (petgraph papier↔tags)
│   ├── pattern_induction.rs    # Induction de motifs
│   ├── falsification.rs        # Falsification d'hypothèses
│   ├── verifier.rs             # Vérification heuristique (anti-random, complexité)
│   ├── probabilistic.rs        # Raisonnement bayésien (statrs)
│   │
│   ├── wasm_executor.rs        # Sandbox wasmtime fuel/epoch + compilation RÉELLE
│   │                           #   Rust→wasm32-unknown-unknown des candidats
│   ├── gpu.rs                  # Détection CUDA/NVIDIA, scoring à poids configurables
│   ├── container.rs            # CacheContainer, KnowledgeContainer, EventContainer
│   ├── queue.rs                # WorkQueue prioritaire, ResultQueue, Backpressure
│   ├── signaling.rs            # EventBus, Signal, Barrier, Latch, Rendezvous
│   ├── probes.rs               # Sonde benchmark : compilation réelle via rustc
│   │                           #   (--emit=metadata, timeout), perf/mémoire estimées
│   └── scientific_contract.rs  # Schémas memorithm.science v1, SHA-256, exports
│                               #   JSON / JSONL / CSV
└── tests/                      # 22 tests d'intégration (dont CLI black-box)
```

### Dépendances clés
| Crate | Rôle |
|-------|------|
| `wasmtime` | Sandbox WASM (fuel + epoch interruption) pour exécution réelle des candidats |
| `ort` | ONNX Runtime (embeddings MiniLM on-device, fallback déterministe) |
| `hnsw_rs` | Index ANN pour recherche vectorielle |
| `metrics` + `metrics-exporter-prometheus` | Observabilité `/metrics` (versions appariées 0.22/0.13) |
| `figment` | Configuration TOML + env vars (`PAPERS_LLM__MODEL=…`) |
| `pdf-extract` | Extraction texte PDF |
| `reqwest` (blocking) | arXiv API, Ollama/OpenAI, CCOS Research Lab |
| `statrs` / `ndarray` / `petgraph` | Statistiques, algèbre, graphes |

---

## 🧬 Fonctionnement

### Pipeline complet (`papers run`)
1. **Extraction** : PDF (`pdf-extract`), arXiv (API Atom), URL HTTP ou texte brut
2. **Analyse** : parsing sémantique puis LLM multi-passes (contributions, résumé, architecture,
   risques, math, pseudo-code). En mode strict, chaque échec LLM est tracé dans
   `llm_warnings` du rapport JSON et rendu visible dans le Markdown — jamais avalé.
3. **Évolution** (`--evolve`) : génération de candidats Rust, évaluation structurelle,
   exécution sandboxée réelle quand la cible wasm32 est installée
4. **Rapports** : `analysis_report.md`, `extracted_document.json`, `evolution_report.md`,
   `best_program.rs`, export PDF possible

### Évaluation des candidats
- **Compilation réelle** : `rustc --target wasm32-unknown-unknown --crate-type=cdylib`
  puis exécution wasmtime avec limite de fuel et deadline epoch
- Convention d'entrée : le candidat définit `pub fn run()` — un adaptateur généré expose
  `#[no_mangle] extern "C" fn main()` qui l'appelle
- Si la cible wasm32 est absente : refus explicite avec instruction
  `rustup target add wasm32-unknown-unknown` (jamais de score inventé)

### Embeddings
- **Défaut** : TF-IDF + signed feature hashing FNV-1a 64 (128-dim), déterministe, testable
- **ONNX** : all-MiniLM-L6-v2 via `ort` quand le modèle est fourni
- **Cache** : `EmbeddingCache` persistant (SHA-256 `dim|texte` → vecteur), écriture atomique,
  tolérant aux corruptions — évite de recalculer entre deux runs

---

## ⚙️ Configuration (`config.toml` / env)

```toml
[llm]
provider = "ollama"          # ou "openai"
model = "gemma4:e2b"         # constante unique DEFAULT_LLM_MODEL
base_url = "http://localhost:11434"
api_key = ""                 # None => aucun header Authorization envoyé
timeout_secs = 300
retry_attempts = 3           # backoff exponentiel sur réseau/5xx
retry_backoff_ms = 500

[gpu.score_weights.training] # poids de sélection GPU configurables
memory_factor = 0.002
compute_high = 15.0
vendor_bonus = 10.0
```

Variables d'environnement : préfixe `PAPERS_`, séparateur `__`
(ex. `PAPERS_LLM__API_KEY=sk-...`).

---

## 📊 Métriques exposées

| Métrique | Sens |
|----------|------|
| `papers_extractions_total` | Documents extraits |
| `papers_analyses_total` | Analyses réalisées |
| `papers_evolutions_total` | Boucles d'évolution lancées |
| `papers_llm_calls_total` / `papers_llm_failures_total` | Trafic LLM |
| `papers_watch_papers_found_total` | Nouveaux papiers détectés par watch |

---

## 🔧 Modèles LLM

Ollama doit être en cours d'exécution (`ollama serve`). Modèles testés :

| Modèle | Taille | Usage |
|--------|--------|-------|
| `gemma4:e2b` | 7.2 Go | Rapide (~68 tok/s), par défaut |
| `qwen3:4b` | 2.5 Go | Ultra-rapide |
| `gemma4:31b` | 19 Go | Haute qualité |
| `qwen3-coder:30b` | 18 Go | Optimisé code |

Toute API OpenAI-compatible fonctionne aussi : `PAPERS_LLM__PROVIDER=openai`,
`PAPERS_LLM__BASE_URL=https://…`, `PAPERS_LLM__API_KEY=…`.

---

## 🛡️ Garanties d'intégrité

- `PaperRegistry` : sauvegarde atomique (tmp + rename), backup `.bak` automatique,
  restauration au chargement, **refus d'écraser** un registre corrompu avec un état vide
  (`force_save()` pour assumer explicitement la perte)
- `ScientificBundle` : SHA-256 du contenu extrait et de l'analyse, identités de claims
  déterministes, validation stricte des schémas v1
- Aucun score empirique fabriqué localement : NAS fail-closed, WASM réel ou refus motivé

---

## 📄 Licence

MIT — CHECKUPAUTO 2024-2026
