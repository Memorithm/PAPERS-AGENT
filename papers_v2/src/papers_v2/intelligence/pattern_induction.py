"""
Pattern Induction Engine
Based on: Jian et al. 2026 "Thinking with Patterns"
         Breslow et al. 2025 "Genomic Next-Token Predictors are In-Context Learners"

Autonomously discovers reusable composite patterns from experimental experience.
Patterns are extracted from evolution history, stored as composable primitives,
and retrieved to guide future hypothesis generation.
"""

from __future__ import annotations

import json
import re
from collections import defaultdict
from typing import Any

import numpy as np
from loguru import logger


class Pattern:
    def __init__(
        self,
        pattern_id: str,
        signature: str,
        description: str,
        source_nodes: list[str],
        conditions: dict[str, Any] | None = None,
        transformations: list[str] | None = None,
        success_rate: float = 0.0,
        usage_count: int = 0,
    ) -> None:
        self.id = pattern_id
        self.signature = signature
        self.description = description
        self.source_nodes = source_nodes
        self.conditions = conditions or {}
        self.transformations = transformations or []
        self.success_rate = success_rate
        self.usage_count = usage_count

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "signature": self.signature,
            "description": self.description,
            "source_nodes": self.source_nodes,
            "conditions": self.conditions,
            "transformations": self.transformations,
            "success_rate": self.success_rate,
            "usage_count": self.usage_count,
        }


class PatternInductionEngine:
    def __init__(self, min_occurrences: int = 2, similarity_threshold: float = 0.7) -> None:
        self.min_occurrences = min_occurrences
        self.similarity_threshold = similarity_threshold
        self.patterns: dict[str, Pattern] = {}
        self.occurrence_index: dict[str, list[str]] = defaultdict(list)

    def induce_from_history(
        self,
        history: list[dict[str, Any]],
        min_score: float = 0.5,
    ) -> list[Pattern]:
        qualified = [h for h in history if h.get("score", 0) >= min_score]
        if len(qualified) < self.min_occurrences:
            logger.debug(f"Pas assez de runs qualifies ({len(qualified)} < {self.min_occurrences})")
            return []

        parent_child = self._extract_parent_child_patterns(qualified)
        structural = self._extract_structural_patterns(qualified)
        convergent = self._extract_convergent_patterns(qualified)

        all_patterns = parent_child + structural + convergent
        logger.info(f"PatternInduction: {len(all_patterns)} patterns decouverts")
        return all_patterns

    def _extract_parent_child_patterns(
        self, history: list[dict[str, Any]]
    ) -> list[Pattern]:
        patterns: list[Pattern] = []
        parent_map: dict[str, list[dict[str, Any]]] = defaultdict(list)

        for h in history:
            parent_id = h.get("parent_id", "root")
            parent_map[parent_id].append(h)

        for parent_id, children in parent_map.items():
            if len(children) < 2:
                continue

            scores = [c["score"] for c in children]
            mean_score = float(np.mean(scores))
            score_std = float(np.std(scores))
            score_improvement = max(scores) - min(scores) if len(scores) > 1 else 0

            if score_improvement > 0.1:
                best_child = max(children, key=lambda x: x["score"])
                patterns.append(Pattern(
                    pattern_id=f"parent_improvement_{parent_id[:8]}",
                    signature=f"parent({parent_id[:8]})->best_child({best_child.get('node_id', '?')[:8]})",
                    description=(
                        f"Parent {parent_id[:8]} yields improvements when modified. "
                        f"Mean child score: {mean_score:.3f} ± {score_std:.3f}, "
                        f"max improvement: {score_improvement:.3f}"
                    ),
                    source_nodes=[c.get("node_id", "") for c in children],
                    conditions={"parent_id": parent_id, "min_improvement": 0.1},
                    success_rate=mean_score,
                    usage_count=len(children),
                ))

        return patterns

    def _extract_structural_patterns(
        self, history: list[dict[str, Any]]
    ) -> list[Pattern]:
        patterns: list[Pattern] = []

        code_blocks = [
            self._extract_code_skeleton(h.get("program", "") + h.get("motivation", ""))
            for h in history
        ]

        block_freq: dict[str, int] = defaultdict(int)
        block_scores: dict[str, list[float]] = defaultdict(list)
        block_nodes: dict[str, list[str]] = defaultdict(list)

        for i, blocks in enumerate(code_blocks):
            score = history[i].get("score", 0)
            node_id = history[i].get("node_id", str(i))
            for block in blocks:
                block_freq[block] += 1
                block_scores[block].append(score)
                block_nodes[block].append(node_id)

        for block, freq in block_freq.items():
            if freq >= self.min_occurrences:
                avg_score = float(np.mean(block_scores[block]))
                if avg_score >= 0.5:
                    patterns.append(Pattern(
                        pattern_id=f"structural_{hash(block) % 100000:05d}",
                        signature=block[:80],
                        description=f"Structural pattern recurring {freq}x, avg score {avg_score:.3f}",
                        source_nodes=block_nodes[block],
                        conditions={"min_frequency": self.min_occurrences},
                        success_rate=avg_score,
                        usage_count=freq,
                    ))

        return patterns

    def _extract_convergent_patterns(
        self, history: list[dict[str, Any]]
    ) -> list[Pattern]:
        patterns: list[Pattern] = []
        if len(history) < 3:
            return patterns

        scores = [h.get("score", 0) for h in history]
        windows = [(i, i + 3) for i in range(len(scores) - 2)]

        for start, end in windows:
            window = scores[start:end]
            if all(window[i] <= window[i + 1] for i in range(len(window) - 1)):
                node_ids = [history[i].get("node_id", str(i)) for i in range(start, end)]
                patterns.append(Pattern(
                    pattern_id=f"convergent_{start}_{end}",
                    signature=f"monotonic_improvement[{start}:{end}]",
                    description=f"Sustained improvement across {end - start} consecutive rounds. "
                    f"Scores: {window[0]:.3f} -> {window[-1]:.3f}",
                    source_nodes=node_ids,
                    conditions={
                        "monotonic": True,
                        "window_size": end - start,
                        "score_delta": window[-1] - window[0],
                    },
                    success_rate=float(np.mean(window)),
                    usage_count=end - start,
                ))

        return patterns

    def find_similar(self, query_pattern: Pattern, top_k: int = 5) -> list[Pattern]:
        scored: list[tuple[Pattern, float]] = []
        query_blocks = set(query_pattern.signature.split())
        query_transforms = set(query_pattern.transformations)

        for pattern in self.patterns.values():
            if pattern.id == query_pattern.id:
                continue

            sig_blocks = set(pattern.signature.split())
            transform_blocks = set(pattern.transformations)

            sig_sim = (
                len(query_blocks & sig_blocks) / max(len(query_blocks | sig_blocks), 1)
            )
            transform_sim = (
                len(query_transforms & transform_blocks)
                / max(len(query_transforms | transform_blocks), 1)
            )
            score_factor = 1.0 - abs(pattern.success_rate - query_pattern.success_rate)

            similarity = 0.4 * sig_sim + 0.3 * transform_sim + 0.3 * score_factor
            if similarity >= self.similarity_threshold:
                scored.append((pattern, similarity))

        scored.sort(key=lambda x: x[1], reverse=True)
        return [p for p, _ in scored[:top_k]]

    def decompose_to_primitives(self, pattern: Pattern) -> list[str]:
        primitives: list[str] = []
        code_patterns = [
            (r"def\s+(\w+)", "function_definition"),
            (r"class\s+(\w+)", "class_definition"),
            (r"(?:self|cls)\.\w+\s*=", "attribute_assignment"),
            (r"for\s+\w+\s+in\s+", "loop_pattern"),
            (r"if\s+.*:\s*$", "conditional_branch"),
            (r"return\s+", "return_statement"),
            (r"import\s+\w+", "import_statement"),
            (r"@\w+", "decorator_pattern"),
        ]
        for regex, label in code_patterns:
            if re.search(regex, pattern.signature, re.MULTILINE):
                primitives.append(label)
        return primitives

    def build_composite(self, primitives: list[str], description: str) -> Pattern:
        signature = " + ".join(primitives)
        composite_id = f"composite_{hash(signature) % 100000:05d}"
        return Pattern(
            pattern_id=composite_id,
            signature=signature,
            description=description,
            source_nodes=[],
            transformations=primitives,
            success_rate=0.5,
            usage_count=0,
        )

    def register(self, pattern: Pattern) -> None:
        self.patterns[pattern.id] = pattern
        for node_id in pattern.source_nodes:
            self.occurrence_index[node_id].append(pattern.id)

    def query_by_node(self, node_id: str) -> list[Pattern]:
        pattern_ids = self.occurrence_index.get(node_id, [])
        return [self.patterns[pid] for pid in pattern_ids if pid in self.patterns]

    def top_patterns(self, k: int = 10) -> list[Pattern]:
        scored = sorted(
            self.patterns.values(),
            key=lambda p: p.success_rate * np.log(max(p.usage_count, 1) + 1),
            reverse=True,
        )
        return scored[:k]

    @staticmethod
    def _extract_code_skeleton(text: str) -> list[str]:
        text = re.sub(r'"""[\s\S]*?"""', "", text)
        text = re.sub(r"'''[\s\S]*?'''", "", text)
        text = re.sub(r"#.*$", "", text, flags=re.MULTILINE)

        skeletons: list[str] = []
        lines = [l.strip() for l in text.split("\n") if l.strip()]

        i = 0
        while i < len(lines):
            if lines[i].startswith("def ") or lines[i].startswith("class "):
                block_lines = [lines[i]]
                i += 1
                while i < len(lines) and (lines[i].startswith((" ", "\t")) or lines[i] == ""):
                    if lines[i]:
                        block_lines.append(lines[i].strip())
                    i += 1
                skeletons.append(" ".join(block_lines)[:120])
            else:
                i += 1

        return skeletons if skeletons else [text[:120] for text in lines[:5]]

    def save(self, path: str) -> None:
        data = {
            "min_occurrences": self.min_occurrences,
            "similarity_threshold": self.similarity_threshold,
            "patterns": {pid: p.to_dict() for pid, p in self.patterns.items()},
        }
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    def load(self, path: str) -> None:
        import os
        if not os.path.exists(path):
            return
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        self.min_occurrences = data.get("min_occurrences", self.min_occurrences)
        self.similarity_threshold = data.get("similarity_threshold", self.similarity_threshold)
        for pid, pd in data.get("patterns", {}).items():
            p = Pattern(
                pattern_id=pid,
                signature=pd["signature"],
                description=pd["description"],
                source_nodes=pd.get("source_nodes", []),
                conditions=pd.get("conditions", {}),
                transformations=pd.get("transformations", []),
                success_rate=pd.get("success_rate", 0.0),
                usage_count=pd.get("usage_count", 0),
            )
            self.patterns[pid] = p
            for node_id in p.source_nodes:
                self.occurrence_index[node_id].append(pid)
        logger.info(f"PatternInductionEngine loaded: {len(self.patterns)} patterns")
