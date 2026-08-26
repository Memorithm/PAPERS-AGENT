# Changelog

Tous les changements notables de ce projet sont documentés dans ce fichier.
Le format s'inspire de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/)
et le versionnage suit [Semantic Versioning](https://semver.org/lang/fr/).

## [0.4.1] - 2026-08-26

### Sécurité
- **wasmtime 25.0.3 → 48.0.1** : corrige 15 advisories RUSTSEC
  (dont sandbox escape aarch64 Cranelift `RUSTSEC-2026-0096`, OOB write
  transcodage component model `RUSTSEC-2026-0091`, fuite de données entre
  instances du pooling allocator `RUSTSEC-2026-0088`).
- **anyhow → 1.0.104** : corrige l'unsoundness de `Error::downcast_mut()`
  (`RUSTSEC-2026-0190`).
- **pdf-extract 0.10 → 0.12** (lopdf 0.38 → 0.42) : corrige le stack overflow
  sur PDF profondément imbriqués (`RUSTSEC-2026-0187`).
- **ort épinglé à `=2.0.0-rc.12`** : évite une résolution implicite vers rc.13,
  dont les binaires précompilés CUDA ne correspondent pas au feature set du projet.

### Ajouté
- CI : cache cargo (registry + target), job `audit` dédié (`cargo audit`),
  étape `cargo doc --no-deps`.
- `CHANGELOG.md` et `SECURITY.md`.

### Modifié
- `reporting::render` refactoré en 15 fonctions de section pures ;
  plus aucune fonction ne dépasse le seuil clippy de complexité cognitive.
- Suppression de `Config::async_support` (déprécié et sans effet depuis wasmtime 48).

### Corrigé
- 2 liens intra-doc cassés dans la rustdoc ([`save`], [`execute`] → [`Self::…`]).
- Branche CI obsolète `feat/scientific-contracts-ccos-rsi` retirée des déclencheurs push.

## [0.4.0] - 2026-08-25

### Ajouté
- Boucle d'évolution Researcher→Engineer→Analyzer avec sampling
  greedy / UCB1 / random / island.
- Exécution réelle des candidats : compilation `rustc --target wasm32-unknown-unknown`
  puis exécution wasmtime (fuel + epoch deadline). Refus motivé si la cible est absente.
- Export scientific bundle v1 (`papers-contract`) : JSON / JSONL / CSV, SHA-256,
  identités de claims déterministes.
- Planificateur d'expériences explicite (`papers-experiment`).
- Commandes CLI `watch` (surveillance arXiv) et `serve-metrics` (Prometheus).
- Embeddings ONNX MiniLM avec fallback TF-IDF déterministe + cache disque SHA-256.
- Mode strict LLM : chaque échec est tracé dans `llm_warnings` (rapport JSON/Markdown).

### Intégrité
- `PaperRegistry` : écriture atomique tmp+rename, backup `.bak`,
  refus d'écraser un registre corrompu.
- NAS fail-closed : aucun score synthétique fabriqué localement.

## [0.3.0] - 2026-07

### Ajouté
- Contrats scientifiques versionnés CCOS/RSI (schéma interchange v1).
- Fixture canonique cross-repo + tests de replay-stabilité.
