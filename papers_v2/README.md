# PAPERS V2

**P**redictive **A**cademic **P**aper **E**xtraction & **E**ngineering **R**esearch **S**ystem — Version 2.

Agent autonome de recherche appliquée spécialisé dans l'analyse de publications scientifiques et leur transformation en spécifications logicielles, prototypes et recommandations d'intégration pour architectures IA autonomes.

## Objectif

Transformer la recherche scientifique brute en :

1. Connaissance structurée
2. Hypothèses d'intégration
3. Modules logiciels implémentables
4. Expériences reproductibles
5. Décisions architecturales
6. Feuilles de route d'implémentation

## Installation

```bash
cd papers_v2
pip install -e ".[dev]"
```

## Utilisation rapide

```bash
# Analyser un papier depuis un lien ArXiv
papers analyze https://arxiv.org/abs/2401.12345

# Analyser un fichier PDF local
papers analyze ./path/to/paper.pdf

# Lancer le serveur du graphe de connaissances
papers knowledge serve
```

## Architecture

```
papers_v2/
├── src/papers_v2/
│   ├── core/           # Modèles, configuration, types
│   ├── extraction/     # Extraction depuis PDF, web, APIs
│   ├── analysis/       # Analyse mathématique, système, scoring
│   ├── transpilation/  # Transpilation concept → code
│   ├── experimentation/# Plans d'expérimentation et tests
│   ├── knowledge/      # Graphe de connaissances et mémoire
│   ├── cli/            # Interface en ligne de commande
│   └── utils/         # Utilitaires
├── tests/             # Tests
├── docs/            # Documentation
├── examples/        # Exemples
└── configs/         # Configurations
```

## Licence

MIT
