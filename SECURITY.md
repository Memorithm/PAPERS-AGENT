# Politique de sécurité

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique.**

Pour signaler une vulnérabilité de sécurité dans PAPERS-AGENT :

1. Utilisez les [rapports privés de sécurité GitHub](https://github.com/Memorithm/PAPERS-AGENT/security/advisories/new)
   (onglet *Security* → *Report a vulnerability*) ; ou
2. Contactez l'équipe maintenance via un canal privé (email de contact du dépôt).

Merci d'inclure :
- une description de la vulnérabilité et de son impact ;
- les étapes de reproduction ou un proof-of-concept ;
- les versions affectées (`papers_core/Cargo.toml`) ;
- les mesures d'atténuation connues, le cas échéant.

## Délais cibles

| Étape | Délai |
|-------|-------|
| Accusé de réception | 48 h |
| Évaluation initiale et sévérité | 7 jours |
| Correctif pour vulnérabilité critique/haute | 30 jours |

## Périmètre

### En périmètre
- Le code Rust de `papers_core` (crate `papers_core`, binaires `papers`,
  `papers-contract`, `papers-experiment`).
- Les workflows CI (`.github/workflows/`).

### Hors périmètre
- Le runtime d'exécution empirique externe (CCOS Research Lab / RSI) —
  signaler à son propre projet.
- Les dépendances : merci de les signaler en amont quand c'est possible ;
  nous suivons `cargo audit` en continu.

## Contexte sécurité spécifique au projet

PAPERS exécute **du code généré par LLM** dans deux contextes cloisonnés :

1. **Sandbox WASM** (`wasm_executor.rs`) — wasmtime avec fuel limit + epoch
   deadline ; compilation réelle via `rustc --target wasm32-unknown-unknown`.
   Toute fuite de sandbox wasmtime est considérée comme critique.
2. **Sondes locales** (`probes.rs`) — invocation `rustc --emit=metadata`
   avec timeout, sans exécution du code candidat.

Le cœur refuse par conception de produire des scores empiriques synthétiques :
`nas.rs` est fail-closed et délègue au runtime externe.

## Divulgation coordonnée

Nous pratiquons la divulgation coordonnée : le correctif est publié avec un
crédit au rapporteur (sauf demande contraire), puis l'advisory publique.
