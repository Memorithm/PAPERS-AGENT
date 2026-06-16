from __future__ import annotations

import re
from pathlib import Path
from typing import Protocol
from urllib.parse import urlparse

import arxiv
import fitz
import httpx
from loguru import logger

from papers_v2.core.models import Author, Domain, Publication
from papers_v2.extraction.semantic import SemanticPaperParser


class SourceExtractor(Protocol):
    def can_handle(self, source: str) -> bool:
        ...

    def extract(self, source: str) -> Publication:
        ...


class ArxivExtractor:
    def can_handle(self, source: str) -> bool:
        return "arxiv.org" in source.lower() or re.match(r"^\d{4}\.\d{4,5}$", source) is not None

    def extract(self, source: str) -> Publication:
        logger.info(f"Extraction ArXiv: {source}")
        client = arxiv.Client()
        if "arxiv.org" in source:
            match = re.search(r"arxiv\.org/abs/(\d+\.\d+)", source)
            if match:
                paper_id = match.group(1)
            else:
                raise ValueError(f"URL ArXiv non reconnue: {source}")
        else:
            paper_id = source

        search = arxiv.Search(id_list=[paper_id])
        results = list(client.results(search))
        if not results:
            raise ValueError(f"Aucun résultat ArXiv pour {paper_id}")

        paper = results[0]
        return Publication(
            id=f"PAPERS-{paper.published.year}-{paper_id.replace('.', '')}",
            title=paper.title,
            authors=[Author(name=a.name) for a in paper.authors],
            publication_date=paper.published.isoformat() if paper.published else None,
            source="arXiv",
            domains=[],
            paper_url=paper.pdf_url,
            github_url=None,
            abstract=paper.summary,
            full_text=None,
            references=[],
        )


class PDFExtractor:
    def can_handle(self, source: str) -> bool:
        parsed = urlparse(source)
        return parsed.scheme in ("", "file") and source.lower().endswith(".pdf")

    def extract(self, source: str) -> Publication:
        logger.info(f"Extraction PDF: {source}")
        path = Path(source)
        if not path.exists():
            raise FileNotFoundError(f"Fichier PDF non trouvé: {source}")

        doc = fitz.open(str(path))
        text = ""
        for page in doc:
            text += page.get_text()

        parser = SemanticPaperParser()
        parsed = parser.parse(text, title=path.stem)

        title = parsed.title or path.stem
        abstract = parsed.abstract or (text[:2000] if text else None)
        github = parsed.github_urls[0] if parsed.github_urls else None
        return Publication(
            id=f"PAPERS-LOCAL-{path.stem.upper()}",
            title=title,
            authors=[],
            publication_date=None,
            source="PDF local",
            domains=[],
            paper_url=None,
            github_url=github,
            abstract=abstract,
            full_text=text,
            references=parsed.references or [],
        )


class PlainTextExtractor:
    def can_handle(self, source: str) -> bool:
        parsed = urlparse(source)
        return parsed.scheme in ("", "file") and (source.lower().endswith(".txt") or source.lower().endswith(".md"))

    def extract(self, source: str) -> Publication:
        logger.info(f"Extraction texte: {source}")
        path = Path(source)
        if not path.exists():
            raise FileNotFoundError(f"Fichier non trouvé: {source}")
        text = path.read_text(encoding="utf-8")
        parser = SemanticPaperParser()
        parsed = parser.parse(text, title=path.stem)
        github = parsed.github_urls[0] if parsed.github_urls else None
        return Publication(
            id=f"PAPERS-LOCAL-{path.stem.upper()}",
            title=parsed.title or path.stem,
            authors=[],
            publication_date=None,
            source="Texte local",
            domains=[],
            paper_url=None,
            github_url=github,
            abstract=parsed.abstract or (text[:2000] if text else None),
            full_text=text,
            references=parsed.references or [],
        )


class URLExtractor:
    def can_handle(self, source: str) -> bool:
        parsed = urlparse(source)
        return parsed.scheme in ("http", "https")

    def extract(self, source: str) -> Publication:
        logger.info(f"Extraction URL: {source}")
        response = httpx.get(source, timeout=30.0)
        response.raise_for_status()
        text = response.text
        return Publication(
            id=f"PAPERS-URL-{hash(source) & 0xFFFFFFFF:08x}",
            title=source,
            authors=[],
            publication_date=None,
            source=source,
            domains=[],
            paper_url=source,
            github_url=None,
            abstract=text[:2000] if text else None,
            full_text=text,
            references=[],
        )


class ExtractionPipeline:
    def __init__(self) -> None:
        self.extractors: list[SourceExtractor] = [
            ArxivExtractor(),
            PDFExtractor(),
            PlainTextExtractor(),
            URLExtractor(),
        ]

    def extract(self, source: str) -> Publication:
        for extractor in self.extractors:
            if extractor.can_handle(source):
                return extractor.extract(source)
        raise ValueError(f"Aucun extracteur ne peut traiter la source: {source}")
