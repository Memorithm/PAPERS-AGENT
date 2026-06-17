use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "papers", about = "PAPERS V2 - Moteur d'évolution autonome en Rust", version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Pipeline complet: extraire → analyser → évoluer
    Run {
        /// Source du papier (PDF, arXiv ID, URL)
        #[arg(short, long)]
        source: String,

        /// Activer l'évolution
        #[arg(short, long, default_value = "false")]
        evolve: bool,

        /// Désactiver le LLM (analyse heuristique uniquement)
        #[arg(long, default_value = "false")]
        no_llm: bool,

        /// Modèle LLM à utiliser
        #[arg(long, default_value = "gemma4:e2b")]
        model: String,

        /// Répertoire de sortie
        #[arg(short, long, default_value = "./output")]
        output: String,

        /// Rounds d'évolution max
        #[arg(long, default_value = "50")]
        rounds: usize,

        /// Sampling policy
        #[arg(long, default_value = "greedy")]
        policy: String,

        /// Candidats par round
        #[arg(short, long, default_value = "3")]
        candidates: usize,
    },

    /// Extraire un papier
    Extract {
        /// Source (PDF, arXiv ID, URL)
        #[arg(short, long)]
        source: String,

        /// Fichier de sortie JSON
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Analyser un papier extrait
    Analyze {
        /// Source (PDF, arXiv ID, URL)
        #[arg(short, long)]
        source: String,

        /// Désactiver le LLM
        #[arg(long, default_value = "false")]
        no_llm: bool,

        /// Modèle LLM
        #[arg(long, default_value = "gemma4:e2b")]
        model: String,

        /// Répertoire de sortie
        #[arg(short, long, default_value = "./output")]
        output: String,
    },

    /// Lancer l'évolution sur une tâche
    Evolve {
        /// Description de la tâche
        #[arg(short, long)]
        task: String,

        /// Source du papier pour contexte (optionnel)
        #[arg(short, long)]
        source: Option<String>,

        /// Rounds max
        #[arg(short, long, default_value = "50")]
        rounds: usize,

        /// Sampling policy
        #[arg(long, default_value = "greedy")]
        policy: String,

        /// Candidats par round
        #[arg(short, long, default_value = "3")]
        candidates: usize,

        /// Modèle LLM
        #[arg(long, default_value = "gemma4:e2b")]
        model: String,

        /// Répertoire de sortie
        #[arg(short, long, default_value = "./output")]
        output: String,
    },

    /// Rechercher des papiers similaires
    Search {
        /// Requête de recherche
        #[arg(short, long)]
        query: String,

        /// Nombre de résultats
        #[arg(short, long, default_value = "5")]
        top_k: usize,
    },

    /// Générer un rapport Markdown
    Report {
        /// Fichier JSON d'analyse
        #[arg(short, long)]
        input: String,

        /// Fichier de sortie Markdown
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Afficher les statistiques de la base
    Status {
        /// Fichier de base de données
        #[arg(long, default_value = "./evolution_db.json")]
        db: String,
    },

    /// Peupler la base de cognition depuis un fichier
    Seed {
        /// Fichier source
        #[arg(short, long)]
        file: String,
    },

    /// Mode interactif (REPL)
    Interactive {
        /// Modèle LLM
        #[arg(long, default_value = "gemma4:e2b")]
        model: String,
    },

    /// Exporter un rapport en PDF
    Pdf {
        /// Fichier JSON d'analyse
        #[arg(short, long)]
        input: String,

        /// Fichier PDF de sortie
        #[arg(short, long)]
        output: Option<String>,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}
