from pathlib import Path
from typing import Any

import yaml

from papers_v2.core.models import AgentConfig


def load_config(path: str | Path | None = None) -> AgentConfig:
    if path is None:
        default = Path("config.yaml")
        if default.exists():
            return AgentConfig.load(default)
        return AgentConfig()
    return AgentConfig.load(path)


def save_config(config: AgentConfig, path: str | Path) -> None:
    config.save(path)
