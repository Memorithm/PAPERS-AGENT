from __future__ import annotations

import json
import re
from typing import Any
from urllib.parse import urlparse

import httpx
from loguru import logger


class GitHubExtractor:
    def __init__(self, token: str | None = None) -> None:
        self.token = token
        self.headers: dict[str, str] = {"Accept": "application/vnd.github+json"}
        if token:
            self.headers["Authorization"] = f"Bearer {token}"

    def extract_repo_info(self, repo_url: str) -> dict[str, Any]:
        match = re.match(r"https?://github\.com/([^/]+)/([^/]+)/?", repo_url)
        if not match:
            raise ValueError(f"URL GitHub invalide: {repo_url}")
        owner, repo = match.group(1), match.group(2)
        repo = repo.replace(".git", "")

        logger.info(f"Extraction infos GitHub: {owner}/{repo}")
        api_url = f"https://api.github.com/repos/{owner}/{repo}"
        response = httpx.get(api_url, headers=self.headers, timeout=30.0)
        response.raise_for_status()
        data = response.json()

        return {
            "owner": owner,
            "repo": repo,
            "stars": data.get("stargazers_count"),
            "forks": data.get("forks_count"),
            "language": data.get("language"),
            "license": data.get("license", {}).get("spdx_id") if data.get("license") else None,
            "updated_at": data.get("updated_at"),
            "topics": data.get("topics", []),
            "has_readme": self._has_file(owner, repo, "README.md"),
            "has_requirements": self._has_file(owner, repo, "requirements.txt"),
            "has_pyproject": self._has_file(owner, repo, "pyproject.toml"),
            "has_setup": self._has_file(owner, repo, "setup.py"),
            "has_tests": self._has_directory(owner, repo, "tests"),
        }

    def extract_benchmarks(self, repo_url: str) -> list[dict[str, Any]]:
        match = re.match(r"https?://github\.com/([^/]+)/([^/]+)/?", repo_url)
        if not match:
            return []
        owner, repo = match.group(1), match.group(2).replace(".git", "")

        # Search for benchmark scripts and result files
        code_files = self._search_files(owner, repo, "benchmark")[:10]
        result_files = self._search_files(owner, repo, "results")[:10]
        return [{"type": "code", "path": f} for f in code_files] + [{"type": "results", "path": f} for f in result_files]

    def _has_file(self, owner: str, repo: str, path: str) -> bool:
        url = f"https://api.github.com/repos/{owner}/{repo}/contents/{path}"
        response = httpx.get(url, headers=self.headers, timeout=10.0)
        return response.status_code == 200

    def _has_directory(self, owner: str, repo: str, path: str) -> bool:
        url = f"https://api.github.com/repos/{owner}/{repo}/contents/{path}"
        response = httpx.get(url, headers=self.headers, timeout=10.0)
        return response.status_code == 200 and isinstance(response.json(), list)

    def _search_files(self, owner: str, repo: str, query: str) -> list[str]:
        url = f"https://api.github.com/search/code"
        params = {"q": f"{query}+repo:{owner}/{repo}"}
        response = httpx.get(url, headers=self.headers, params=params, timeout=30.0)
        if response.status_code != 200:
            return []
        data = response.json()
        return [item.get("path", "") for item in data.get("items", [])]


class BenchmarkExtractor:
    def __init__(self) -> None:
        self.common_datasets = [
            "pg19", "proof-pile", "c4", "the pile", "pile", "wikitext", "hellaswag",
            "mmlu", "gsm8k", "swag", "squad", "glue", "superglue", "enwik8", "lambada",
            "arc", "boolq", "piqa", "winogrande", "openbookqa"
        ]

    def extract_from_text(self, text: str) -> list[dict[str, str]]:
        benchmarks: list[dict[str, str]] = []
        seen = set()
        for ds in self.common_datasets:
            for m in re.finditer(rf"\b{re.escape(ds)}\b", text, re.IGNORECASE):
                if ds in seen:
                    continue
                seen.add(ds)
                start = max(0, m.start() - 50)
                end = min(len(text), m.end() + 100)
                snippet = text[start:end].strip()
                benchmarks.append({
                    "dataset": ds,
                    "context": snippet,
                })
        return benchmarks
