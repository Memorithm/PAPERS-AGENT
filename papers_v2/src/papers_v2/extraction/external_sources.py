from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any
from urllib.parse import quote, urlparse

import httpx
from loguru import logger

from papers_v2.core.models import Author, Publication


class SemanticScholarExtractor:
    API_BASE = "https://api.semanticscholar.org/graph/v1"

    def __init__(self, api_key: str | None = None) -> None:
        self.api_key = api_key
        self.headers: dict[str, str] = {}
        if api_key:
            self.headers["x-api-key"] = api_key

    def search(self, query: str, limit: int = 5) -> list[Publication]:
        url = f"{self.API_BASE}/paper/search"
        params = {
            "query": query,
            "fields": "title,authors,year,abstract,url,openAccessPdf,externalIds,publicationDate",
            "limit": limit,
        }
        response = httpx.get(url, params=params, headers=self.headers, timeout=30.0)
        response.raise_for_status()
        data = response.json()

        publications: list[Publication] = []
        for paper in data.get("data", []):
            pub = self._paper_to_publication(paper)
            if pub:
                publications.append(pub)
        return publications

    def by_id(self, paper_id: str) -> Publication | None:
        if paper_id.startswith("arXiv:"):
            paper_id = f"ARXIV:{paper_id.split(':')[1]}"
        url = f"{self.API_BASE}/paper/{paper_id}"
        params = {
            "fields": "title,authors,year,abstract,url,openAccessPdf,externalIds,publicationDate",
        }
        response = httpx.get(url, params=params, headers=self.headers, timeout=30.0)
        if response.status_code == 404:
            return None
        response.raise_for_status()
        return self._paper_to_publication(response.json())

    def _paper_to_publication(self, paper: dict[str, Any]) -> Publication | None:
        if not paper.get("title"):
            return None
        authors = [
            Author(name=a.get("name", "Unknown"))
            for a in paper.get("authors", [])
        ]
        pdf_url = None
        oa = paper.get("openAccessPdf")
        if oa and isinstance(oa, dict):
            pdf_url = oa.get("url")
        if not pdf_url:
            pdf_url = paper.get("url")

        external_ids = paper.get("externalIds") or {}
        arxiv_id = external_ids.get("ArXiv")

        pub_id = f"PAPERS-{paper.get('year', '0000')}-{hash(paper.get('title')) & 0xFFFFFFFF:08x}"
        return Publication(
            id=pub_id,
            title=paper["title"],
            authors=authors,
            publication_date=paper.get("publicationDate"),
            source="Semantic Scholar",
            domains=[],
            paper_url=pdf_url,
            github_url=None,
            abstract=paper.get("abstract"),
            full_text=None,
            references=[],
        )


class HuggingFacePapersExtractor:
    RSS_URL = "https://huggingface.co/papers/feed.xml"
    API_BASE = "https://huggingface.co/api/papers"

    def latest(self, limit: int = 10) -> list[Publication]:
        logger.info("Récupération des derniers papiers Hugging Face...")
        response = httpx.get(self.API_BASE, params={"limit": limit}, timeout=30.0)
        response.raise_for_status()
        data = response.json()

        publications: list[Publication] = []
        for item in data:
            pub = self._item_to_publication(item)
            if pub:
                publications.append(pub)
        return publications

    def _item_to_publication(self, item: dict[str, Any]) -> Publication | None:
        paper = item.get("paper", {})
        title = paper.get("title") or item.get("title")
        if not title:
            return None
        authors = [Author(name=a) for a in paper.get("authors", [])]
        published = item.get("publishedAt")
        return Publication(
            id=f"PAPERS-HF-{hash(title) & 0xFFFFFFFF:08x}",
            title=title,
            authors=authors,
            publication_date=published,
            source="Hugging Face Papers",
            domains=[],
            paper_url=paper.get("url") or item.get("url"),
            github_url=None,
            abstract=item.get("summary"),
            full_text=None,
            references=[],
        )


class OpenReviewExtractor:
    API_BASE = "https://api2.openreview.net"

    def search(self, query: str, limit: int = 5) -> list[Publication]:
        url = f"{self.API_BASE}/notes/search"
        params = {
            "term": query,
            "type": "forum",
            "limit": limit,
        }
        response = httpx.get(url, params=params, timeout=30.0)
        response.raise_for_status()
        data = response.json()

        publications: list[Publication] = []
        for note in data.get("notes", []):
            content = note.get("content", {})
            title = content.get("title", "")
            if not title:
                continue
            authors_list = content.get("authors", {}).get("value", []) if isinstance(content.get("authors"), dict) else content.get("authors", [])
            authors = [Author(name=a) for a in authors_list]
            publications.append(Publication(
                id=f"PAPERS-OPENREVIEW-{note.get('id', 'unknown')}",
                title=title,
                authors=authors,
                publication_date=note.get("tcdate"),
                source="OpenReview",
                domains=[],
                paper_url=f"https://openreview.net/forum?id={note.get('forum', '')}",
                github_url=None,
                abstract=content.get("abstract", {}).get("value") if isinstance(content.get("abstract"), dict) else content.get("abstract"),
                full_text=None,
                references=[],
            ))
        return publications
