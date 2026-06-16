"""
Configuration adaptee de l'implementation ASI-Evolve.
YAML deep merge avec resolution de variables d'environnement.
"""

from __future__ import annotations

import os
from copy import deepcopy
from pathlib import Path
from typing import Any

import yaml


def deep_merge(base: dict, override: dict) -> dict:
    result = deepcopy(base)
    for key, value in override.items():
        if key in result and isinstance(result[key], dict) and isinstance(value, dict):
            result[key] = deep_merge(result[key], value)
        else:
            result[key] = deepcopy(value)
    return result


def load_config(
    config_path: str | Path | None = None,
    experiment_name: str | None = None,
    project_root: str | Path | None = None,
) -> dict[str, Any]:
    root = Path(project_root or Path.cwd())

    config: dict[str, Any] = {}

    default_config = root / "config.yaml"
    if default_config.exists():
        with open(default_config, encoding="utf-8") as f:
            config = yaml.safe_load(f) or {}

    if experiment_name:
        exp_config = root / "experiments" / experiment_name / "config.yaml"
        if exp_config.exists():
            with open(exp_config, encoding="utf-8") as f:
                config = deep_merge(config, yaml.safe_load(f) or {})

    if config_path:
        cp = Path(config_path)
        if not cp.exists():
            raise FileNotFoundError(f"Config file not found: {config_path}")
        with open(cp, encoding="utf-8") as f:
            config = deep_merge(config, yaml.safe_load(f) or {})

    return _resolve_env_vars(config)


def _resolve_env_vars(obj: Any) -> Any:
    if isinstance(obj, dict):
        return {k: _resolve_env_vars(v) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [_resolve_env_vars(item) for item in obj]
    elif isinstance(obj, str) and obj.startswith("${") and obj.endswith("}"):
        return os.environ.get(obj[2:-1], "")
    return obj
