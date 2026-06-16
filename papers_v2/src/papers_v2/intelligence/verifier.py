"""
Verifier Module - Counterexample-Guided Verification
Based on: Liu et al. 2026 "Counterexample Guided Learning"
         NOETHER (Li et al. 2026)

Provides rich structured feedback beyond scalar scores.
Generates counterexamples, checks invariants, validates constraints.
"""

from __future__ import annotations

import ast
import hashlib
import json
from typing import Any

import numpy as np


class VerificationReport:
    def __init__(
        self,
        candidate_id: str,
        passed: bool = True,
        score: float = 0.0,
        counterexamples: list[dict[str, Any]] | None = None,
        violations: list[str] | None = None,
        suggestions: list[str] | None = None,
        invariant_checks: dict[str, bool] | None = None,
    ) -> None:
        self.candidate_id = candidate_id
        self.passed = passed
        self.score = score
        self.counterexamples = counterexamples or []
        self.violations = violations or []
        self.suggestions = suggestions or []
        self.invariant_checks = invariant_checks or {}

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "passed": self.passed,
            "score": self.score,
            "counterexamples": self.counterexamples,
            "violations": self.violations,
            "suggestions": self.suggestions,
            "invariant_checks": self.invariant_checks,
        }

    def to_rich_feedback(self) -> str:
        parts = []
        if self.counterexamples:
            parts.append("## Counterexamples Found\n")
            for ce in self.counterexamples[:3]:
                parts.append(f"- **Input**: {ce.get('input', 'N/A')}")
                parts.append(f"  **Expected**: {ce.get('expected', 'N/A')}")
                parts.append(f"  **Got**: {ce.get('actual', 'N/A')}\n")

        if self.violations:
            parts.append("## Violations\n")
            for v in self.violations:
                parts.append(f"- {v}\n")

        if self.suggestions:
            parts.append("## Suggestions\n")
            for s in self.suggestions:
                parts.append(f"- {s}\n")

        return "\n".join(parts) if parts else "All checks passed."


class CounterexampleGuidedVerifier:
    def __init__(self, llm_client: Any = None) -> None:
        self.llm = llm_client

    def verify(
        self,
        candidate_id: str,
        program: str,
        results: dict[str, Any],
        constraints: list[dict[str, Any]] | None = None,
    ) -> VerificationReport:
        counterexamples = self._find_counterexamples(program, results)
        violations = self._check_constraints(program, constraints or [])
        invariant_checks = self._verify_invariants(program, results)
        suggestions = self._generate_suggestions(
            program, counterexamples, violations, invariant_checks
        )

        passed = len(counterexamples) == 0 and len(violations) == 0

        score = self._compute_verification_score(
            counterexamples, violations, invariant_checks
        )

        return VerificationReport(
            candidate_id=candidate_id,
            passed=passed,
            score=score,
            counterexamples=counterexamples,
            violations=violations,
            suggestions=suggestions,
            invariant_checks=invariant_checks,
        )

    def _find_counterexamples(
        self, program: str, results: dict[str, Any]
    ) -> list[dict[str, Any]]:
        counterexamples: list[dict[str, Any]] = []

        error_msg = results.get("error", results.get("stderr", ""))
        if error_msg:
            counterexamples.append({
                "input": "runtime_execution",
                "expected": "successful execution",
                "actual": error_msg[:300],
                "type": "runtime_error",
            })

        stdout = str(results.get("stdout", ""))
        import re
        traceback_match = re.search(r"Traceback[\s\S]*?Error: (.+?)(?:\n|$)", stdout)
        if traceback_match:
            counterexamples.append({
                "input": "program_execution",
                "expected": "no errors",
                "actual": traceback_match.group(1)[:200],
                "type": "traceback_error",
            })

        output = results.get("output", results.get("result"))
        if isinstance(output, (int, float)) and output == 0:
            counterexamples.append({
                "input": "default_case",
                "expected": "non-zero output",
                "actual": 0,
                "type": "zero_output",
            })

        return counterexamples

    def _check_constraints(
        self, program: str, constraints: list[dict[str, Any]]
    ) -> list[str]:
        violations: list[str] = []

        for constraint in constraints:
            constraint_type = constraint.get("type", "")
            target = constraint.get("target", "")
            expected = constraint.get("expected", "")

            if constraint_type == "contains_function" and target:
                if f"def {target}" not in program:
                    violations.append(f"Missing required function: {target}")

            elif constraint_type == "contains_import" and target:
                if f"import {target}" not in program:
                    violations.append(f"Missing required import: {target}")

            elif constraint_type == "max_complexity":
                try:
                    tree = ast.parse(program)
                    complexity = sum(
                        1 for node in ast.walk(tree)
                        if isinstance(node, (ast.If, ast.For, ast.While, ast.FunctionDef))
                    )
                    max_val = constraint.get("value", 50)
                    if complexity > max_val:
                        violations.append(
                            f"Complexity {complexity} exceeds max {max_val}"
                        )
                except SyntaxError:
                    violations.append("Program has syntax errors")

        return violations

    def _verify_invariants(
        self, program: str, results: dict[str, Any]
    ) -> dict[str, bool]:
        checks: dict[str, bool] = {}

        checks["no_crash"] = results.get("success", False)

        runtime = results.get("runtime_seconds", 0)
        checks["within_timeout"] = runtime < 3600

        output = results.get("output", results.get("result"))
        if isinstance(output, (int, float)):
            checks["finite_output"] = np.isfinite(output)
            checks["non_negative"] = output >= 0
        else:
            checks["finite_output"] = True
            checks["non_negative"] = True

        checks["deterministic"] = self._check_determinism(program)

        return checks

    def _check_determinism(self, program: str) -> bool:
        non_deterministic = [
            "random.", "np.random", "torch.rand",
            "time.time()", "uuid.", "os.urandom",
        ]
        return not any(nd in program for nd in non_deterministic)

    def _generate_suggestions(
        self,
        program: str,
        counterexamples: list[dict[str, Any]],
        violations: list[str],
        invariant_checks: dict[str, bool],
    ) -> list[str]:
        suggestions: list[str] = []

        for ce in counterexamples:
            if ce.get("type") == "runtime_error":
                suggestions.append(
                    f"Fix runtime error: {ce['actual'][:100]}. "
                    "Add error handling around the failing operation."
                )
            elif ce.get("type") == "zero_output":
                suggestions.append(
                    "Output is zero for default case. Verify input handling and "
                    "ensure the computation path is reached."
                )

        for v in violations:
            if "Missing required function" in v:
                suggestions.append(f"Add function: {v.split(': ')[-1]}")
            elif "syntax errors" in v:
                suggestions.append("Fix syntax errors. Run syntax check before submission.")

        if not invariant_checks.get("within_timeout", True):
            suggestions.append(
                "Execution exceeds timeout. Optimize loops, reduce input size, "
                "or add early termination conditions."
            )

        if not invariant_checks.get("deterministic", True):
            suggestions.append(
                "Program uses non-deterministic operations. Consider seeding random "
                "generators for reproducibility."
            )

        if not suggestions:
            all_passed = all(invariant_checks.values()) if invariant_checks else True
            if all_passed:
                suggestions.append(
                    "All checks passed. Consider more aggressive optimization "
                    "or exploring alternative architectural patterns."
                )

        return suggestions

    def _compute_verification_score(
        self,
        counterexamples: list[dict[str, Any]],
        violations: list[str],
        invariant_checks: dict[str, bool],
    ) -> float:
        base_score = 1.0

        base_score -= 0.2 * len(counterexamples)
        base_score -= 0.15 * len(violations)

        if invariant_checks:
            pass_rate = sum(invariant_checks.values()) / len(invariant_checks)
            base_score = 0.5 * base_score + 0.5 * pass_rate

        return max(0.0, min(1.0, base_score))

    def batch_verify(
        self,
        candidates: list[dict[str, Any]],
        results_list: list[dict[str, Any]],
        constraints: list[dict[str, Any]] | None = None,
    ) -> list[VerificationReport]:
        reports = []
        for i, (candidate, results) in enumerate(zip(candidates, results_list)):
            cid = candidate.get("id", f"candidate_{i}")
            program = candidate.get("program", "")
            report = self.verify(cid, program, results, constraints)
            reports.append(report)
        return reports
