from __future__ import annotations

from pathlib import Path
from typing import Any

import chromadb
from chromadb.config import Settings
from loguru import logger
from sentence_transformers import SentenceTransformer

from papers_v2.core.models import AnalysisReport, Publication


class PaperVectorStore:
    def __init__(
        self,
        persist_dir: str | Path = "./vector_store",
        model_name: str = "sentence-transformers/all-MiniLM-L6-v2",
    ) -> None:
        self.persist_dir = Path(persist_dir)
        self.persist_dir.mkdir(parents=True, exist_ok=True)
        self.model_name = model_name
        self.client = chromadb.PersistentClient(path=str(self.persist_dir))
        self.collection = self.client.get_or_create_collection("papers")
        self.model: SentenceTransformer | None = None

    def _load_model(self) -> SentenceTransformer:
        if self.model is None:
            logger.info(f"Chargement du modèle d'embeddings {self.model_name}...")
            self.model = SentenceTransformer(self.model_name)
        return self.model

    def add_report(self, report: AnalysisReport, extra_metadata: dict[str, Any] | None = None) -> None:
        publication = report.publication
        doc = self._document_from_report(report)
        if not doc:
            logger.warning("Document vide, ignoré.")
            return

        embedding = self._embed(doc).tolist()
        metadata = {
            "id": publication.id,
            "title": publication.title,
            "source": publication.source or "",
            "github_url": publication.github_url or "",
            "integration_score": report.integration.score,
            "reproducibility_score": report.reproducibility.score,
            "recommendation": report.recommendation.value,
        }
        if extra_metadata:
            metadata.update(extra_metadata)

        self.collection.add(
            ids=[publication.id],
            embeddings=[embedding],
            metadatas=[metadata],
            documents=[doc],
        )
        logger.info(f"Ajouté au store vectoriel: {publication.id}")

    def search(
        self,
        query: str,
        n_results: int = 5,
        filters: dict[str, Any] | None = None,
    ) -> list[dict[str, Any]]:
        embedding = self._embed(query).tolist()
        kwargs: dict[str, Any] = {
            "query_embeddings": [embedding],
            "n_results": n_results,
        }
        if filters:
            kwargs["where"] = filters
        results = self.collection.query(**kwargs)
        output: list[dict[str, Any]] = []
        ids = results.get("ids", [[]])[0]
        distances = results.get("distances", [[]])[0]
        metadatas = results.get("metadatas", [[]])[0]
        documents = results.get("documents", [[]])[0]
        for i in range(len(ids)):
            output.append({
                "id": ids[i],
                "distance": distances[i] if i < len(distances) else None,
                "metadata": metadatas[i] if i < len(metadatas) else {},
                "document": documents[i] if i < len(documents) else "",
            })
        return output

    def find_similar_papers(
        self,
        report: AnalysisReport,
        n_results: int = 5,
    ) -> list[dict[str, Any]]:
        doc = self._document_from_report(report)
        if not doc:
            return []
        # exclude self
        results = self.search(doc, n_results=n_results + 1)
        return [r for r in results if r["id"] != report.publication.id][:n_results]

    def _embed(self, text: str) -> Any:
        model = self._load_model()
        return model.encode(text, show_progress_bar=False, convert_to_numpy=True)

    def _document_from_report(self, report: AnalysisReport) -> str:
        parts = [
            report.publication.title,
            report.publication.abstract or "",
            " ".join(report.publication.domains),
            report.executive_summary or "",
            " ".join(report.scientific_contributions),
            " ".join(report.architectural_mapping.impacted_modules),
        ]
        return "\n".join(p for p in parts if p).strip()

    def persist(self) -> None:
        if hasattr(self.client, "persist"):
            self.client.persist()
            logger.info("Store vectoriel persisté.")
