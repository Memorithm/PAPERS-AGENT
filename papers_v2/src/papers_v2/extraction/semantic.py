from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

from loguru import logger


@dataclass
class ParsedPaper:
    title: str | None = None
    abstract: str | None = None
    sections: dict[str, str] | None = None
    equations: list[str] | None = None
    variables: list[dict[str, str]] | None = None
    datasets: list[str] | None = None
    metrics: list[str] | None = None
    limitations: list[str] | None = None
    github_urls: list[str] | None = None
    references: list[str] | None = None


class SemanticPaperParser:
    def parse(self, text: str, title: str | None = None) -> ParsedPaper:
        logger.info("Parsing sémantique du texte...")
        sections = self._extract_sections(text)
        abstract = sections.get("abstract", "") or self._extract_abstract(text)
        return ParsedPaper(
            title=title,
            abstract=abstract,
            sections=sections,
            equations=self._extract_equations(text),
            variables=self._extract_variables(text),
            datasets=self._extract_datasets(text),
            metrics=self._extract_metrics(text),
            limitations=self._extract_limitations(text),
            github_urls=self._extract_github_urls(text),
            references=self._extract_references(text),
        )

    def _extract_sections(self, text: str) -> dict[str, str]:
        sections: dict[str, str] = {}
        patterns = [
            r"(?im)^\s*(abstract|résumé)\s*$",
            r"(?im)^\s*(introduction)\s*$",
            r"(?im)^\s*(related work|état de l'art|travaux connexes)\s*$",
            r"(?im)^\s*(method|methods|méthode|méthodes|methodology)\s*$",
            r"(?im)^\s*(experiments|expériences|experimental setup)\s*$",
            r"(?im)^\s*(results|résultats)\s*$",
            r"(?im)^\s*(discussion)\s*$",
            r"(?im)^\s*(conclusion|conclusions)\s*$",
            r"(?im)^\s*(limitations|limites)\s*$",
            r"(?im)^\s*(references|références)\s*$",
        ]
        section_names = ["abstract", "introduction", "related work", "method", "experiments",
                         "results", "discussion", "conclusion", "limitations", "references"]

        matches: list[tuple[int, str]] = []
        for pattern, name in zip(patterns, section_names):
            for m in re.finditer(pattern, text):
                matches.append((m.start(), name))
        matches.sort()

        for i, (start, name) in enumerate(matches):
            end = matches[i + 1][0] if i + 1 < len(matches) else len(text)
            sections[name] = text[start:end].strip()
        return sections

    def _extract_abstract(self, text: str) -> str:
        m = re.search(r"(?is)abstract[\s:]*(.{100,2000})\n\s*\n", text)
        if m:
            return m.group(1).strip()
        return ""

    def _extract_equations(self, text: str) -> list[str]:
        inline = re.findall(r"\$[^$]+?\$", text)
        display = re.findall(r"\$\$[^$]+?\$\$", text)
        return list(set(inline + display))[:50]

    def _extract_variables(self, text: str) -> list[dict[str, str]]:
        vars_found: list[dict[str, str]] = []
        seen = set()
        for m in re.finditer(r"([A-Za-z][A-Za-z0-9_]*)\s*=\s*([^\n,;]+)", text):
            name = m.group(1).strip()
            if name in seen or len(name) <= 1:
                continue
            seen.add(name)
            vars_found.append({"name": name, "meaning": m.group(2).strip()})
        return vars_found[:30]

    def _extract_datasets(self, text: str) -> list[str]:
        keywords = [
            "dataset", "datasets", "benchmark", "corpus", "pg19", "proof-pile",
            "c4", "the pile", "pile", "wikitext", "hellaswag", "mmlu", "gsm8k",
            "swag", "squad", "glue", "superglue", "enwik8", "lambada", "arc",
            "boolq", "piqa", "winogrande", "openbookqa"
        ]
        found: set[str] = set()
        for kw in keywords:
            for m in re.finditer(rf"\b{re.escape(kw)}\b", text, re.IGNORECASE):
                # Extract surrounding context
                start = max(0, m.start() - 30)
                end = min(len(text), m.end() + 30)
                snippet = text[start:end].strip()
                found.add(snippet)
        return sorted(list(found))[:20]

    def _extract_metrics(self, text: str) -> list[str]:
        keywords = [
            "perplexity", "bleu", "rouge", "accuracy", "f1", "precision", "recall",
            "latency", "throughput", "flops", "params", "memory", "vram", "perplexité",
            "exact match", "em", "mrr", "ndcg", "map"
        ]
        found: set[str] = set()
        for kw in keywords:
            for m in re.finditer(rf"\b{re.escape(kw)}\b", text, re.IGNORECASE):
                start = max(0, m.start() - 25)
                end = min(len(text), m.end() + 40)
                found.add(text[start:end].strip())
        return sorted(list(found))[:20]

    def _extract_limitations(self, text: str) -> list[str]:
        section = re.search(r"(?is)(limitations|limites|weaknesses)\s*[\n:](.+?)(?=\n\s*(references|références|conclusion|appendix|acknowledgements)\s*$|$)", text)
        if section:
            raw = section.group(2)
            sentences = re.split(r"(?<=[.!?])\s+", raw)
            return [s.strip() for s in sentences if len(s.strip()) > 20][:10]
        return []

    def _extract_github_urls(self, text: str) -> list[str]:
        return re.findall(r"https?://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/?", text)

    def _extract_references(self, text: str) -> list[str]:
        refs = re.findall(r"\[\d+\]\s*(.+?)(?=\n\[|\Z)", text, re.DOTALL)
        return [r.strip().replace("\n", " ") for r in refs[:50]]
