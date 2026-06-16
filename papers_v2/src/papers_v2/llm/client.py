from __future__ import annotations

import json
import re
from typing import Any

import httpx
from loguru import logger


class OllamaClient:
    def __init__(self, base_url: str = "http://localhost:11434", model: str = "gemma4:e2b", timeout: float = 120.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.timeout = timeout

    def is_available(self) -> bool:
        try:
            response = httpx.get(f"{self.base_url}/api/tags", timeout=5.0)
            return response.status_code == 200
        except Exception:
            return False

    def generate(
        self,
        prompt: str,
        system: str | None = None,
        temperature: float = 0.3,
        max_tokens: int = 2048,
        json_mode: bool = False,
    ) -> str:
        payload: dict[str, Any] = {
            "model": self.model,
            "prompt": prompt,
            "stream": False,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens,
            },
        }
        if system:
            payload["system"] = system
        if json_mode:
            payload["format"] = "json"

        logger.debug(f"Requête Ollama: {self.model} - prompt len={len(prompt)}")
        response = httpx.post(
            f"{self.base_url}/api/generate",
            json=payload,
            timeout=self.timeout,
        )
        response.raise_for_status()
        data = response.json()

        # Some models (e.g., qwen3) put JSON in "thinking" field
        response_text = data.get("response", "").strip()
        thinking_text = data.get("thinking", "").strip()
        return response_text or thinking_text

    def generate_json(
        self,
        prompt: str,
        system: str | None = None,
        temperature: float = 0.2,
        max_tokens: int = 2048,
    ) -> dict[str, Any]:
        raw = self.generate(prompt, system=system, temperature=temperature, max_tokens=max_tokens, json_mode=True)
        try:
            return json.loads(raw)
        except json.JSONDecodeError as e:
            logger.warning(f"JSON invalide du LLM: {e}. Tentative d'extraction...")
            return self._extract_json(raw)

    @staticmethod
    def _extract_json(text: str) -> dict[str, Any]:
        # Try to extract JSON from markdown code blocks or raw text
        patterns = [
            r"```json\s*(\{.*?\})\s*```",
            r"```\s*(\{.*?\})\s*```",
            r"(\{[\s\S]*?\})",
        ]
        for pattern in patterns:
            matches = re.findall(pattern, text, re.DOTALL)
            for m in matches:
                try:
                    return json.loads(m)
                except json.JSONDecodeError:
                    continue
        return {}
