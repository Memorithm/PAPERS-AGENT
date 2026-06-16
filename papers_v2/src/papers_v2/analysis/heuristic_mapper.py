from __future__ import annotations

import re

from papers_v2.core.models import AnalysisReport, ArchitecturalMapping


class HeuristicArchitecturalMapper:
    def map(self, report: AnalysisReport) -> ArchitecturalMapping:
        text = " ".join(filter(None, [
            report.publication.title,
            report.publication.abstract,
            report.executive_summary,
            " ".join(report.scientific_contributions),
            report.publication.full_text or "",
        ])).lower()

        mapping = ArchitecturalMapping()

        keywords = {
            "memory": ["memory", "mémoire", "kv cache", "long-term memory", "retrieval", "rag", "store", "recall"],
            "perception": ["perception", "embedding", "representation", "encoding", "encoder", "feature"],
            "planning": ["planning", "planification", "plan", "search", "mcts", "tree", "subgoal"],
            "decision": ["decision", "policy", "choix", "reasoning", "raisonnement", "inference"],
            "action": ["action", "tool use", "act", "execution", "environment", "agent"],
            "learning": ["learning", "apprentissage", "training", "fine-tuning", "distillation", "update"],
            "reflection": ["reflection", "reflect", "self-correct", "self-evaluation", "introspection"],
            "evaluation": ["evaluation", "eval", "benchmark", "metric", "reward", "score"],
        }

        for pillar, kws in keywords.items():
            matches = [kw for kw in kws if kw in text]
            if matches:
                getattr(mapping, pillar).extend(matches)

        impacted = []
        if mapping.memory:
            impacted.append("memory_module")
        if mapping.perception:
            impacted.append("perception_module")
        if mapping.decision or mapping.planning or mapping.evaluation:
            impacted.append("reasoning_module")
        if mapping.planning:
            impacted.append("planning_module")
        if mapping.action:
            impacted.append("action_module")
        if mapping.learning:
            impacted.append("learning_module")
        if mapping.reflection:
            impacted.append("reflection_module")
        if mapping.evaluation:
            impacted.append("evaluation_module")

        mapping.impacted_modules = impacted
        mapping.required_interfaces = [f"{m}_api" for m in impacted]
        mapping.dependencies = self._detect_dependencies(text)
        return mapping

    def _detect_dependencies(self, text: str) -> list[str]:
        deps = []
        if "pytorch" in text or "torch" in text:
            deps.append("PyTorch")
        if "jax" in text:
            deps.append("JAX")
        if "tensorflow" in text:
            deps.append("TensorFlow")
        if "cuda" in text:
            deps.append("CUDA")
        if "rust" in text:
            deps.append("Rust")
        if "c++" in text or "cpp" in text:
            deps.append("C++")
        if "transformers" in text:
            deps.append("Hugging Face Transformers")
        if "flash attention" in text:
            deps.append("Flash Attention")
        return deps
