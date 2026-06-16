"""
LLM Client - OpenAI-compatible + Ollama avec fallback.
Basé sur l'implementation officielle ASI-Evolve, adaptée pour PAPERS V2.
"""

from __future__ import annotations

import json
import re
import time
from typing import Any

import httpx
from loguru import logger

# Try OpenAI client, fallback to Ollama
try:
    from openai import OpenAI
    HAS_OPENAI = True
except ImportError:
    HAS_OPENAI = False


class LLMClient:
    def __init__(
        self,
        provider: str = "ollama",
        model: str = "gemma4:e2b",
        base_url: str = "http://localhost:11434",
        api_key: str = "EMPTY",
        timeout: int = 120,
        retry_times: int = 3,
        retry_delay: int = 5,
        temperature: float = 0.3,
        max_tokens: int = 4096,
    ) -> None:
        self.provider = provider
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout
        self.retry_times = retry_times
        self.retry_delay = retry_delay
        self.temperature = temperature
        self.max_tokens = max_tokens

        if provider == "openai" and HAS_OPENAI:
            self._client = OpenAI(api_key=api_key, base_url=base_url, timeout=timeout)
        else:
            self._client = None

    def is_available(self) -> bool:
        if self.provider == "ollama":
            try:
                response = httpx.get(f"{self.base_url}/api/tags", timeout=5.0)
                return response.status_code == 200
            except Exception:
                return False
        elif self.provider == "openai" and self._client:
            return True
        return False

    def generate(
        self,
        prompt: str,
        system: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
        json_mode: bool = False,
    ) -> str:
        for attempt in range(self.retry_times):
            try:
                if self.provider == "ollama":
                    return self._generate_ollama(prompt, system, temperature, max_tokens, json_mode)
                elif self.provider == "openai" and self._client:
                    return self._generate_openai(prompt, system, temperature, max_tokens, json_mode)
            except Exception as e:
                logger.warning(f"LLM call failed (attempt {attempt + 1}/{self.retry_times}): {e}")
                if attempt < self.retry_times - 1:
                    time.sleep(self.retry_delay)
        raise RuntimeError(f"LLM generation failed after {self.retry_times} attempts")

    def _generate_ollama(
        self, prompt: str, system: str | None, temperature: float | None,
        max_tokens: int | None, json_mode: bool,
    ) -> str:
        payload: dict[str, Any] = {
            "model": self.model,
            "prompt": prompt,
            "stream": False,
            "options": {
                "temperature": temperature or self.temperature,
                "num_predict": max_tokens or self.max_tokens,
            },
        }
        if system:
            payload["system"] = system
        if json_mode:
            payload["format"] = "json"

        response = httpx.post(
            f"{self.base_url}/api/generate",
            json=payload,
            timeout=self.timeout,
        )
        response.raise_for_status()
        data = response.json()
        return data.get("response", "") or data.get("thinking", "")

    def _generate_openai(
        self, prompt: str, system: str | None, temperature: float | None,
        max_tokens: int | None, json_mode: bool,
    ) -> str:
        assert self._client is not None
        messages: list[dict[str, str]] = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})

        kwargs: dict[str, Any] = {
            "model": self.model,
            "messages": messages,
            "temperature": temperature or self.temperature,
            "max_tokens": max_tokens or self.max_tokens,
        }
        if json_mode:
            kwargs["response_format"] = {"type": "json_object"}

        response = self._client.chat.completions.create(**kwargs)
        return response.choices[0].message.content or ""

    def generate_json(
        self,
        prompt: str,
        system: str | None = None,
        temperature: float | None = None,
        max_tokens: int | None = None,
    ) -> dict[str, Any]:
        raw = self.generate(prompt, system, temperature, max_tokens, json_mode=True)
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return self._extract_json(raw)

    def extract_tags(self, prompt: str, system: str | None = None) -> dict[str, str]:
        response = self.generate(prompt, system, json_mode=False)
        result: dict[str, str] = {}
        tag_pattern = r"<(\w+)>"
        pos = 0
        content = response.strip()
        while True:
            match = re.search(tag_pattern, content[pos:])
            if not match:
                break
            tag_name = match.group(1)
            tag_start = pos + match.end()
            end_tag = f"</{tag_name}>"
            end_pos = content.find(end_tag, tag_start)
            if end_pos == -1:
                pos = tag_start
                continue
            result[tag_name] = content[tag_start:end_pos].strip()
            pos = end_pos + len(end_tag)
        return result

    @staticmethod
    def _extract_json(text: str) -> dict[str, Any]:
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


def create_llm_client(
    provider: str = "ollama",
    model: str = "gemma4:e2b",
    base_url: str = "http://localhost:11434",
    api_key: str = "EMPTY",
    temperature: float = 0.3,
    max_tokens: int = 4096,
) -> LLMClient:
    return LLMClient(
        provider=provider,
        model=model,
        base_url=base_url,
        api_key=api_key,
        temperature=temperature,
        max_tokens=max_tokens,
    )
