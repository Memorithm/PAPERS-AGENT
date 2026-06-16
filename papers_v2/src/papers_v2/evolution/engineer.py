from __future__ import annotations

import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

from loguru import logger


class Engineer:
    def __init__(
        self,
        timeout_seconds: int = 3600,
        early_reject_timeout: int = 60,
        work_dir: str | Path | None = None,
    ) -> None:
        self.timeout_seconds = timeout_seconds
        self.early_reject_timeout = early_reject_timeout
        self.work_dir = Path(work_dir) if work_dir else Path(tempfile.mkdtemp(prefix="asi_evolve_"))

    def execute(
        self,
        program: str,
        eval_command: str | None = None,
        eval_function: Callable[[str], dict[str, Any]] | None = None,
        early_reject_check: Callable[[str], bool] | None = None,
    ) -> dict[str, Any]:
        self.work_dir.mkdir(parents=True, exist_ok=True)

        if eval_function:
            return self._execute_function(program, eval_function)
        elif eval_command:
            return self._execute_command(program, eval_command, early_reject_check)
        else:
            logger.warning("Engineer: ni commande ni fonction d'evaluation fournie.")
            return {
                "success": False,
                "error": "No evaluation method provided",
                "score": 0.0,
                "metrics": {},
                "runtime_seconds": 0.0,
            }

    def _execute_command(
        self,
        program: str,
        eval_command: str,
        early_reject_check: Callable[[str], bool] | None = None,
    ) -> dict[str, Any]:
        program_path = self.work_dir / "candidate_program.py"
        program_path.write_text(program, encoding="utf-8")

        start_time = time.time()

        if early_reject_check:
            try:
                result = subprocess.run(
                    eval_command,
                    shell=True,
                    capture_output=True,
                    text=True,
                    timeout=self.early_reject_timeout,
                    cwd=str(self.work_dir),
                )
                if result.returncode != 0 and early_reject_check(result.stderr):
                    elapsed = time.time() - start_time
                    return {
                        "success": False,
                        "error": f"Early rejection: {result.stderr[:500]}",
                        "score": 0.0,
                        "metrics": {"early_reject": True},
                        "stdout": result.stdout[:1000],
                        "stderr": result.stderr[:1000],
                        "runtime_seconds": elapsed,
                    }
            except subprocess.TimeoutExpired:
                elapsed = time.time() - start_time
                return {
                    "success": False,
                    "error": "Early rejection timeout",
                    "score": 0.0,
                    "metrics": {"early_reject": True, "timeout": True},
                    "runtime_seconds": elapsed,
                }

        try:
            result = subprocess.run(
                eval_command,
                shell=True,
                capture_output=True,
                text=True,
                timeout=self.timeout_seconds,
                cwd=str(self.work_dir),
            )
            elapsed = time.time() - start_time
            return {
                "success": result.returncode == 0,
                "error": result.stderr[:1000] if result.returncode != 0 else "",
                "score": 0.0,
                "metrics": {"exit_code": result.returncode},
                "stdout": result.stdout[:5000],
                "stderr": result.stderr[:1000],
                "runtime_seconds": elapsed,
            }
        except subprocess.TimeoutExpired:
            elapsed = time.time() - start_time
            return {
                "success": False,
                "error": f"Execution timeout after {self.timeout_seconds}s",
                "score": 0.0,
                "metrics": {"timeout": True},
                "runtime_seconds": elapsed,
            }

    def _execute_function(
        self,
        program: str,
        eval_function: Callable[[str], dict[str, Any]],
    ) -> dict[str, Any]:
        start_time = time.time()
        try:
            result = eval_function(program)
            elapsed = time.time() - start_time
            result.setdefault("runtime_seconds", elapsed)
            result.setdefault("success", True)
            return result
        except Exception as e:
            elapsed = time.time() - start_time
            return {
                "success": False,
                "error": str(e),
                "score": 0.0,
                "metrics": {},
                "runtime_seconds": elapsed,
            }

    @staticmethod
    def compute_fitness(
        metrics: dict[str, Any],
        primary_metric: str = "score",
        llm_judge_score: float | None = None,
        llm_weight: float = 0.2,
    ) -> float:
        primary = float(metrics.get(primary_metric, 0.0))
        if llm_judge_score is not None:
            return primary * (1 - llm_weight) + llm_judge_score * llm_weight
        return primary

    def cleanup(self) -> None:
        import shutil
        if self.work_dir.exists():
            shutil.rmtree(self.work_dir, ignore_errors=True)
