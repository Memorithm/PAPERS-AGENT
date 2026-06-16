"""
Falsification Engine
Based on: Bertolazzi et al. 2026 "FALSIFYBENCH"
         Helff et al. 2026 "LLMs Gaming Verifiers"

Prioritizes hypothesis DISPROOF over confirmation.
Generates falsification tests for each candidate,
ranks by falsifiability, and detects reward hacking.
"""

from __future__ import annotations

import ast
import hashlib
import random
from dataclasses import dataclass, field
from typing import Any, Callable


@dataclass
class FalsificationTest:
    test_id: str
    hypothesis_id: str
    description: str
    input_perturbation: dict[str, Any] = field(default_factory=dict)
    expected_behavior: str = ""
    edge_case: bool = False
    adversarial: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "test_id": self.test_id,
            "hypothesis_id": self.hypothesis_id,
            "description": self.description,
            "input_perturbation": self.input_perturbation,
            "expected_behavior": self.expected_behavior,
            "edge_case": self.edge_case,
            "adversarial": self.adversarial,
        }


@dataclass
class FalsificationResult:
    test: FalsificationTest
    passed: bool
    actual_behavior: str = ""
    confidence: float = 0.0

    def to_dict(self) -> dict[str, Any]:
        return {
            "test_id": self.test.test_id,
            "passed": self.passed,
            "actual_behavior": self.actual_behavior,
            "confidence": self.confidence,
        }


class FalsificationEngine:
    def __init__(self, llm_client: Any = None) -> None:
        self.llm = llm_client
        self.tests: dict[str, list[FalsificationTest]] = {}
        self.results: dict[str, list[FalsificationResult]] = {}

    def generate_tests(
        self,
        hypothesis_id: str,
        hypothesis_text: str,
        program_code: str,
        n_tests: int = 5,
    ) -> list[FalsificationTest]:
        tests: list[FalsificationTest] = []

        tests.extend(self._generate_boundary_tests(hypothesis_id, program_code))
        tests.extend(self._generate_invariance_tests(hypothesis_id, program_code))
        tests.extend(self._generate_adversarial_tests(hypothesis_id, hypothesis_text))
        tests.extend(self._generate_edge_case_tests(hypothesis_id, program_code))

        if self.llm:
            llm_tests = self._generate_llm_tests(hypothesis_id, hypothesis_text, program_code)
            tests.extend(llm_tests)

        tests = self._deduplicate(tests)
        self.tests[hypothesis_id] = tests[:n_tests]
        return tests[:n_tests]

    def _generate_boundary_tests(
        self, hypothesis_id: str, program_code: str
    ) -> list[FalsificationTest]:
        tests: list[FalsificationTest] = []
        boundaries = [
            ("empty_input", "What happens with empty/missing inputs?", {"input_size": 0}),
            ("max_input", "What happens at maximum input size?", {"input_size": 10**6}),
            ("negative_values", "What happens with negative values?", {"value": -1.0}),
            ("zero_values", "What happens with zero values?", {"value": 0.0}),
            ("extreme_values", "What happens with extreme float values?", {"value": float("inf")}),
            ("None_input", "What happens with None inputs?", {"value": None}),
        ]
        for name, desc, perturbation in boundaries:
            tests.append(FalsificationTest(
                test_id=f"{hypothesis_id}_{name}",
                hypothesis_id=hypothesis_id,
                description=desc,
                input_perturbation=perturbation,
                expected_behavior="should handle gracefully or raise appropriate error",
                edge_case=True,
            ))
        return tests

    def _generate_invariance_tests(
        self, hypothesis_id: str, program_code: str
    ) -> list[FalsificationTest]:
        tests: list[FalsificationTest] = []
        invariances = [
            ("permutation", "Does result change under input permutation?", {"shuffle": True}),
            ("scaling", "Does result scale correctly under input scaling?", {"scale": 2.0}),
            ("noise", "Is result robust to small Gaussian noise?", {"noise_std": 0.01}),
            ("repetition", "Is result consistent on repeated identical inputs?", {"repeat": 5}),
        ]
        for name, desc, perturbation in invariances:
            tests.append(FalsificationTest(
                test_id=f"{hypothesis_id}_inv_{name}",
                hypothesis_id=hypothesis_id,
                description=desc,
                input_perturbation=perturbation,
                expected_behavior="output should maintain key invariants",
            ))
        return tests

    def _generate_adversarial_tests(
        self, hypothesis_id: str, hypothesis_text: str
    ) -> list[FalsificationTest]:
        tests: list[FalsificationTest] = []

        claim_patterns = [
            (r"(?:always|never|guaranteed|certain)", "universal_claim"),
            (r"(?:outperforms|better than|superior to)", "comparative_claim"),
            (r"(?:all|every|any|no)\s+\w+", "quantified_claim"),
            (r"(?:optimal|best|minimum|maximum)", "optimality_claim"),
            (r"significantly\s+(?:improves|reduces|increases)", "significance_claim"),
        ]

        import re
        for pattern, claim_type in claim_patterns:
            if re.search(pattern, hypothesis_text, re.IGNORECASE):
                tests.append(FalsificationTest(
                    test_id=f"{hypothesis_id}_adv_{claim_type}",
                    hypothesis_id=hypothesis_id,
                    description=f"Adversarial test for {claim_type}: find counterexample to '{pattern}'",
                    input_perturbation={"mode": "adversarial_search"},
                    expected_behavior="claim should be falsifiable if false",
                    adversarial=True,
                ))

        return tests

    def _generate_edge_case_tests(
        self, hypothesis_id: str, program_code: str
    ) -> list[FalsificationTest]:
        tests: list[FalsificationTest] = []
        tree = None
        try:
            tree = ast.parse(program_code)
        except SyntaxError:
            pass

        if tree:
            for node in ast.walk(tree):
                if isinstance(node, (ast.If, ast.While)):
                    tests.append(FalsificationTest(
                        test_id=f"{hypothesis_id}_branch_{node.lineno}",
                        hypothesis_id=hypothesis_id,
                        description=f"Test branch condition at line {node.lineno}",
                        input_perturbation={"target_branch": node.lineno},
                        expected_behavior="both branches should be reachable",
                        edge_case=True,
                    ))

        return tests

    def _generate_llm_tests(
        self, hypothesis_id: str, hypothesis_text: str, program_code: str
    ) -> list[FalsificationTest]:
        if not self.llm:
            return []

        prompt = (
            f"Given this hypothesis:\n{hypothesis_text[:1000]}\n\n"
            f"And this code:\n{program_code[:1000]}\n\n"
            f"Generate 3 adversarial tests that could FALSIFY the hypothesis. "
            f"Respond as JSON: [{{\"description\": \"...\", \"input\": {{...}}, \"expected\": \"...\"}}]"
        )
        try:
            result = self.llm.generate_json(prompt, max_tokens=1024)
            tests: list[FalsificationTest] = []
            if isinstance(result, list):
                for item in result[:3]:
                    tests.append(FalsificationTest(
                        test_id=f"{hypothesis_id}_llm_{hashlib.md5(item['description'].encode()).hexdigest()[:8]}",
                        hypothesis_id=hypothesis_id,
                        description=item.get("description", ""),
                        input_perturbation=item.get("input", {}),
                        expected_behavior=item.get("expected", ""),
                        adversarial=True,
                    ))
            return tests
        except Exception:
            return []

    def _deduplicate(self, tests: list[FalsificationTest]) -> list[FalsificationTest]:
        seen: set[str] = set()
        unique: list[FalsificationTest] = []
        for t in tests:
            key = t.description[:100]
            if key not in seen:
                seen.add(key)
                unique.append(t)
        return unique

    def evaluate(
        self,
        hypothesis_id: str,
        eval_function: Callable[[dict[str, Any]], dict[str, Any]],
    ) -> list[FalsificationResult]:
        results: list[FalsificationResult] = []
        tests = self.tests.get(hypothesis_id, [])

        for test in tests:
            try:
                outcome = eval_function(test.input_perturbation)
                passed = outcome.get("success", False)
                results.append(FalsificationResult(
                    test=test,
                    passed=passed,
                    actual_behavior=str(outcome.get("output", outcome.get("error", "")))[:200],
                    confidence=0.8 if not test.adversarial else 0.5,
                ))
            except Exception as e:
                results.append(FalsificationResult(
                    test=test,
                    passed=False,
                    actual_behavior=str(e)[:200],
                    confidence=1.0,
                ))

        self.results[hypothesis_id] = results
        return results

    def compute_falsifiability_score(self, hypothesis_id: str) -> float:
        results = self.results.get(hypothesis_id, [])
        if not results:
            return 0.5

        adversarial_results = [r for r in results if r.test.adversarial]
        edge_results = [r for r in results if r.test.edge_case]

        if adversarial_results:
            adv_pass_rate = sum(r.passed for r in adversarial_results) / len(adversarial_results)
        else:
            adv_pass_rate = 1.0

        if edge_results:
            edge_pass_rate = sum(r.passed for r in edge_results) / len(edge_results)
        else:
            edge_pass_rate = 1.0

        overall_pass_rate = sum(r.passed for r in results) / len(results)

        return 1.0 - (0.5 * overall_pass_rate + 0.3 * adv_pass_rate + 0.2 * edge_pass_rate)

    def detect_reward_hacking(
        self,
        hypothesis_id: str,
        score: float,
        falsifiability: float,
    ) -> dict[str, Any]:
        ratio = falsifiability / max(score, 1e-6)

        if ratio > 2.0:
            risk = "HIGH"
            explanation = (
                f"Score {score:.3f} is very high but falsifiability {falsifiability:.3f} is low. "
                "Candidate may be overfitting to the metric rather than learning general patterns. "
                "Recommend isomorphic perturbation testing."
            )
        elif ratio > 1.0:
            risk = "MEDIUM"
            explanation = "Moderate discrepancy between score and falsifiability. Monitor closely."
        else:
            risk = "LOW"
            explanation = "Score and falsifiability are well-aligned. Genuine pattern learning likely."

        return {
            "hypothesis_id": hypothesis_id,
            "score": score,
            "falsifiability": falsifiability,
            "ratio": ratio,
            "risk": risk,
            "explanation": explanation,
        }

    def get_summary(self, hypothesis_id: str) -> dict[str, Any]:
        results = self.results.get(hypothesis_id, [])
        tests = self.tests.get(hypothesis_id, [])

        return {
            "total_tests": len(tests),
            "total_results": len(results),
            "pass_rate": (
                sum(r.passed for r in results) / len(results) if results else 0
            ),
            "adversarial_tests": len([t for t in tests if t.adversarial]),
            "edge_case_tests": len([t for t in tests if t.edge_case]),
            "falsifiability": self.compute_falsifiability_score(hypothesis_id),
            "failures": [
                {"test": r.test.description, "actual": r.actual_behavior}
                for r in results if not r.passed
            ],
        }
