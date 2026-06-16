from __future__ import annotations

from typing import Any

from loguru import logger

from papers_v2.infra.llm_client import LLMClient as InfraLLMClient


class OllamaClient:
    """Wrapper retrocompatible autour du LLMClient infra."""

    def __init__(self, base_url: str = "http://localhost:11434", model: str = "gemma4:e2b", timeout: float = 120.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.timeout = timeout
        self._client = InfraLLMClient(
            provider="ollama",
            model=model,
            base_url=base_url,
            timeout=int(timeout),
            temperature=0.3,
            max_tokens=2048,
        )

    def is_available(self) -> bool:
        return self._client.is_available()

    def generate(
        self,
        prompt: str,
        system: str | None = None,
        temperature: float = 0.3,
        max_tokens: int = 2048,
        json_mode: bool = False,
    ) -> str:
        result = self._client.generate(
            prompt=prompt,
            system=system,
            temperature=temperature,
            max_tokens=max_tokens,
            json_mode=json_mode,
        )
        # Ollama fallback: check "thinking" field
        return result

    def generate_json(
        self,
        prompt: str,
        system: str | None = None,
        temperature: float = 0.2,
        max_tokens: int = 2048,
    ) -> dict[str, Any]:
        return self._client.generate_json(
            prompt=prompt,
            system=system,
            temperature=temperature,
            max_tokens=max_tokens,
        )

    @staticmethod
    def _extract_json(text: str) -> dict[str, Any]:
        return InfraLLMClient._extract_json(text)
