from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any

from pydantic import BaseModel, Field
import yaml


class Domain(str, Enum):
    LLM_ARCHITECTURES = "LLM Architectures"
    TRANSFORMER_OPTIMIZATION = "Transformer Optimization"
    AGENT_ARCHITECTURES = "Agent Architectures"
    MEMORY_SYSTEMS = "Memory Systems"
    RECURSIVE_SELF_IMPROVEMENT = "Recursive Self-Improvement"
    REPRESENTATION_ENGINEERING = "Representation Engineering"
    MECHANISTIC_INTERPRETABILITY = "Mechanistic Interpretability"
    LOCAL_INFERENCE_OPTIMIZATION = "Local Inference Optimization"
    NEURAL_MEMORY = "Neural Memory"
    COGNITIVE_ARCHITECTURES = "Cognitive Architectures"
    AI_INFRASTRUCTURE = "AI Infrastructure"
    DISTRIBUTED_AI_SYSTEMS = "Distributed AI Systems"
    AUTONOMOUS_SOFTWARE_ENGINEERING = "Autonomous Software Engineering"


class ClaimType(str, Enum):
    FACT_PAPER = "[FACT PAPER]"
    DERIVED_ANALYSIS = "[DERIVED ANALYSIS]"
    ENGINEERING_PROPOSAL = "[ENGINEERING PROPOSAL]"
    UNCERTAINTY = "[UNCERTAINTY]"


class RiskLevel(str, Enum):
    LOW = "LOW"
    MEDIUM = "MEDIUM"
    HIGH = "HIGH"
    CRITICAL = "CRITICAL"


class Recommendation(str, Enum):
    REJECT = "REJET"
    ARCHIVE = "ARCHIVAGE"
    FURTHER_RESEARCH = "RECHERCHE SUPPLEMENTAIRE"
    PROTOTYPE = "PROTOTYPE"
    INTEGRATION = "INTEGRATION"


class Author(BaseModel):
    name: str
    affiliation: str | None = None
    email: str | None = None
    orcid: str | None = None


class Publication(BaseModel):
    id: str
    title: str
    authors: list[Author]
    publication_date: str | None = None
    source: str | None = None
    domains: list[Domain] = Field(default_factory=list)
    paper_url: str | None = None
    github_url: str | None = None
    abstract: str | None = None
    full_text: str | None = None
    references: list[str] = Field(default_factory=list)


class ExtractedClaim(BaseModel):
    text: str
    claim_type: ClaimType
    source_section: str | None = None
    confidence: float = Field(ge=0.0, le=1.0, default=1.0)


class MathematicalObject(BaseModel):
    name: str
    meaning: str
    dimensions: str | None = None
    domain: str | None = None


class MathematicalAnalysis(BaseModel):
    equations: list[str] = Field(default_factory=list)
    variables: list[MathematicalObject] = Field(default_factory=list)
    loss_functions: list[str] = Field(default_factory=list)
    transformations: list[str] = Field(default_factory=list)
    uncertainties: list[str] = Field(default_factory=list)


class AlgorithmStep(BaseModel):
    name: str
    description: str
    inputs: list[str] = Field(default_factory=list)
    outputs: list[str] = Field(default_factory=list)
    complexity: str | None = None


class AlgorithmAnalysis(BaseModel):
    steps: list[AlgorithmStep] = Field(default_factory=list)
    pseudocode: str | None = None
    overall_complexity: str | None = None
    memory_cost: str | None = None


class SystemAnalysis(BaseModel):
    vram_consumption: str | None = None
    ram_consumption: str | None = None
    disk_io: str | None = None
    memory_bandwidth: str | None = None
    latency: str | None = None
    throughput: str | None = None
    scalability: str | None = None
    bottlenecks: list[str] = Field(default_factory=list)
    contention_points: list[str] = Field(default_factory=list)
    fragmentation_risks: list[str] = Field(default_factory=list)


class ArchitecturalMapping(BaseModel):
    perception: list[str] = Field(default_factory=list)
    memory: list[str] = Field(default_factory=list)
    planning: list[str] = Field(default_factory=list)
    decision: list[str] = Field(default_factory=list)
    action: list[str] = Field(default_factory=list)
    learning: list[str] = Field(default_factory=list)
    reflection: list[str] = Field(default_factory=list)
    evaluation: list[str] = Field(default_factory=list)
    impacted_modules: list[str] = Field(default_factory=list)
    required_interfaces: list[str] = Field(default_factory=list)
    dependencies: list[str] = Field(default_factory=list)


class ReproducibilityScore(BaseModel):
    documentation: float = Field(ge=0.0, le=1.0)
    code_available: float = Field(ge=0.0, le=1.0)
    data_available: float = Field(ge=0.0, le=1.0)
    results_reproducible: float = Field(ge=0.0, le=1.0)
    acceptable_cost: float = Field(ge=0.0, le=1.0)

    @property
    def score(self) -> float:
        return (
            self.documentation * 0.20
            + self.code_available * 0.25
            + self.data_available * 0.20
            + self.results_reproducible * 0.20
            + self.acceptable_cost * 0.15
        )


class IntegrationScore(BaseModel):
    reproducibility: float = Field(ge=0.0, le=1.0)
    architectural_impact: float = Field(ge=0.0, le=1.0)
    hardware_cost: float = Field(ge=0.0, le=1.0)
    code_availability: float = Field(ge=0.0, le=1.0)
    scientific_maturity: float = Field(ge=0.0, le=1.0)

    @property
    def score(self) -> float:
        return (
            self.reproducibility * 0.25
            + self.architectural_impact * 0.25
            + self.hardware_cost * 0.20
            + self.code_availability * 0.20
            + self.scientific_maturity * 0.10
        )

    @property
    def interpretation(self) -> str:
        if self.score <= 0.30:
            return "NON PRIORITAIRE"
        elif self.score <= 0.70:
            return "EXPERIMENTATION"
        else:
            return "INTEGRATION POSSIBLE"


class Risk(BaseModel):
    description: str
    level: RiskLevel
    mitigation: str | None = None


class ExperimentPlan(BaseModel):
    objective: str | None = None
    hypothesis: str | None = None
    baseline: str | None = None
    dataset: str | None = None
    metrics: list[str] = Field(default_factory=list)
    hardware: str | None = None
    success_criteria: list[str] = Field(default_factory=list)
    failure_criteria: list[str] = Field(default_factory=list)
    estimated_duration: str | None = None
    estimated_cost: str | None = None


class TestPlan(BaseModel):
    unit_tests: list[str] = Field(default_factory=list)
    integration_tests: list[str] = Field(default_factory=list)
    regression_tests: list[str] = Field(default_factory=list)


class SoftwareArtifact(BaseModel):
    language: str
    code: str
    description: str | None = None


class SoftwareSpecification(BaseModel):
    structs: list[SoftwareArtifact] = Field(default_factory=list)
    interfaces: list[SoftwareArtifact] = Field(default_factory=list)
    apis: list[SoftwareArtifact] = Field(default_factory=list)
    algorithms: list[SoftwareArtifact] = Field(default_factory=list)
    complexity_notes: list[str] = Field(default_factory=list)


class AnalysisReport(BaseModel):
    publication: Publication
    executive_summary: str | None = None
    scientific_contributions: list[str] = Field(default_factory=list)
    mathematical_analysis: MathematicalAnalysis = Field(default_factory=MathematicalAnalysis)
    algorithm_analysis: AlgorithmAnalysis = Field(default_factory=AlgorithmAnalysis)
    system_analysis: SystemAnalysis = Field(default_factory=SystemAnalysis)
    architectural_mapping: ArchitecturalMapping = Field(default_factory=ArchitecturalMapping)
    software_specification: SoftwareSpecification = Field(default_factory=SoftwareSpecification)
    experiment_plan: ExperimentPlan = Field(default_factory=ExperimentPlan)
    test_plan: TestPlan = Field(default_factory=TestPlan)
    reproducibility: ReproducibilityScore = Field(default_factory=lambda: ReproducibilityScore(
        documentation=0.0, code_available=0.0, data_available=0.0,
        results_reproducible=0.0, acceptable_cost=0.0
    ))
    integration: IntegrationScore = Field(default_factory=lambda: IntegrationScore(
        reproducibility=0.0, architectural_impact=0.0, hardware_cost=0.0,
        code_availability=0.0, scientific_maturity=0.0
    ))
    risks: list[Risk] = Field(default_factory=list)
    critical_analysis: dict[str, list[str]] = Field(default_factory=dict)
    recommendation: Recommendation = Recommendation.FURTHER_RESEARCH
    recommendation_justification: str | None = None
    claims: list[ExtractedClaim] = Field(default_factory=list)


class AgentConfig(BaseModel):
    target_gpus: list[str] = Field(default_factory=lambda: ["RTX 4090", "RTX 5090"])
    target_ram_gb: tuple[int, int] = (64, 256)
    target_storage: str = "NVMe faible latence"
    target_os: str = "Linux"
    target_languages: list[str] = Field(default_factory=lambda: ["Rust", "Python", "CUDA"])
    max_vram_gb: int = 24
    arxiv_rate_limit: float = 3.0
    output_format: str = "markdown"
    knowledge_graph_path: str = "./knowledge_graph.json"

    @classmethod
    def load(cls, path: str | Path) -> "AgentConfig":
        path = Path(path)
        if not path.exists():
            return cls()
        with open(path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
        return cls(**data)

    def save(self, path: str | Path) -> None:
        path = Path(path)
        with open(path, "w", encoding="utf-8") as f:
            yaml.dump(self.model_dump(), f, default_flow_style=False)


# ── ASI-Evolve Models ──────────────────────────────────────────────

class CognitionItem(BaseModel):
    id: str
    content: str
    source: str = ""
    item_type: str = "heuristic"
    tags: list[str] = Field(default_factory=list)
    embedding: list[float] | None = None


class EvolutionNodeModel(BaseModel):
    id: str
    program: str
    motivation: str = ""
    results: dict[str, Any] = Field(default_factory=dict)
    analysis: str = ""
    score: float = 0.0
    metadata: dict[str, Any] = Field(default_factory=dict)
    visit_count: int = 0


class EvolutionConfig(BaseModel):
    task_description: str = ""
    model: str = "gemma4:e2b"
    sampling_policy: str = "greedy"
    max_rounds: int = 50
    target_score: float | None = None
    patience: int = 10
    n_candidates_per_round: int = 3
    n_context_nodes: int = 5
    n_cognition: int = 5
    db_path: str = "./evolution_db.json"
    cognition_path: str = "./cognition_store"
    output_dir: str = "./evolution_output"
    timeout_seconds: int = 3600
    early_reject_timeout: int = 60

    @classmethod
    def load(cls, path: str | Path) -> "EvolutionConfig":
        path = Path(path)
        if not path.exists():
            return cls()
        with open(path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
        return cls(**data)

    def save(self, path: str | Path) -> None:
        path = Path(path)
        with open(path, "w", encoding="utf-8") as f:
            yaml.dump(self.model_dump(), f, default_flow_style=False)


class EvolutionResult(BaseModel):
    success: bool = False
    best_score: float = 0.0
    best_node: EvolutionNodeModel | None = None
    total_rounds: int = 0
    total_candidates: int = 0
    database_stats: dict[str, Any] = Field(default_factory=dict)
    cognition_count: int = 0
    total_time_seconds: float = 0.0
    stopped_early: bool = False
    history: list[dict[str, Any]] = Field(default_factory=list)
