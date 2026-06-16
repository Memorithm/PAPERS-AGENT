# ⚠️  Migration Rust en cours — modules Python résiduels uniquement
from papers_v2.core.config import load_config
from papers_v2.core.models import AgentConfig, AnalysisReport, Publication
from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode, SamplingPolicy
from papers_v2.evolution.researcher import Researcher
from papers_v2.evolution.engineer import Engineer
from papers_v2.evolution.analyzer import EvolutionAnalyzer
from papers_v2.evolution.loop import EvolutionLoop
from papers_v2.infra.llm_client import LLMClient, create_llm_client
from papers_v2.infra.config import load_config as load_yaml_config, deep_merge
from papers_v2.infra.faiss_index import FAISSIndex
from papers_v2.infra.embedding import EmbeddingService
from papers_v2.infra.samplers import get_sampler
from papers_v2.infra.experiments import circle_packing_evaluator, create_experiment_template
from papers_v2.intelligence.frenzy import ResearchFrenzy
from papers_v2.knowledge.papers_registry import PaperRegistry

__all__ = [
    "AgentConfig", "AnalysisReport",
    "circle_packing_evaluator", "CognitionBase", "CognitionEntry",
    "create_experiment_template", "create_llm_client", "deep_merge",
    "EmbeddingService", "Engineer", "EvolutionAnalyzer",
    "EvolutionDatabase", "EvolutionLoop", "EvolutionNode",
    "FAISSIndex", "get_sampler",
    "LLMClient",
    "load_config", "load_yaml_config",
    "PaperRegistry", "Publication",
    "ResearchFrenzy", "Researcher", "SamplingPolicy",
]

__version__ = "0.4.0"
