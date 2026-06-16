from __future__ import annotations

import json
import uuid
from pathlib import Path
from typing import Any

import chromadb
from chromadb.config import Settings
from loguru import logger
from sentence_transformers import SentenceTransformer


class CognitionEntry:
    def __init__(
        self,
        content: str,
        source: str = "",
        entry_type: str = "heuristic",
        tags: list[str] | None = None,
        entry_id: str | None = None,
    ) -> None:
        self.id = entry_id or str(uuid.uuid4())[:8]
        self.content = content
        self.source = source
        self.entry_type = entry_type
        self.tags = tags or []

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "content": self.content,
            "source": self.source,
            "entry_type": self.entry_type,
            "tags": self.tags,
        }


class CognitionBase:
    def __init__(
        self,
        persist_dir: str | Path = "./cognition_store",
        model_name: str = "sentence-transformers/all-MiniLM-L6-v2",
    ) -> None:
        self.persist_dir = Path(persist_dir)
        self.persist_dir.mkdir(parents=True, exist_ok=True)
        self.model_name = model_name
        self.client = chromadb.PersistentClient(path=str(self.persist_dir))
        self.collection = self.client.get_or_create_collection("cognition")
        self.model: SentenceTransformer | None = None
        self._entries: dict[str, CognitionEntry] = {}

    def _load_model(self) -> SentenceTransformer:
        if self.model is None:
            logger.info(f"Chargement du modele d'embeddings {self.model_name} pour Cognition...")
            self.model = SentenceTransformer(self.model_name)
        return self.model

    def add_entry(self, entry: CognitionEntry) -> None:
        embedding = self._embed(entry.content).tolist()
        self.collection.add(
            ids=[entry.id],
            embeddings=[embedding],
            metadatas=[{
                "source": entry.source,
                "entry_type": entry.entry_type,
                "tags": ",".join(entry.tags),
            }],
            documents=[entry.content],
        )
        self._entries[entry.id] = entry
        logger.debug(f"Cognition: ajout entree {entry.id}")

    def add_entries(self, entries: list[CognitionEntry]) -> None:
        for entry in entries:
            self.add_entry(entry)

    def add_from_papers(
        self,
        papers: list[dict[str, str]],
        entry_type: str = "paper_insight",
    ) -> None:
        for paper in papers:
            content = paper.get("content", "")
            source = paper.get("source", paper.get("title", ""))
            tags = paper.get("tags", [])
            entry = CognitionEntry(
                content=content,
                source=source,
                entry_type=entry_type,
                tags=tags,
            )
            self.add_entry(entry)

    def retrieve(
        self,
        query: str,
        n_results: int = 5,
        min_distance: float | None = None,
    ) -> list[CognitionEntry]:
        if not query.strip():
            return []
        if self.collection.count() == 0:
            return []
        embedding = self._embed(query).tolist()
        n = min(n_results, self.collection.count())
        results = self.collection.query(
            query_embeddings=[embedding],
            n_results=n,
        )
        entries: list[CognitionEntry] = []
        ids = results.get("ids", [[]])[0]
        distances = results.get("distances", [[]])[0]
        metadatas = results.get("metadatas", [[]])[0]
        documents = results.get("documents", [[]])[0]

        for i in range(len(ids)):
            if min_distance is not None and i < len(distances) and distances[i] > min_distance:
                continue
            meta = metadatas[i] if i < len(metadatas) else {}
            tags = meta.get("tags", "").split(",") if meta.get("tags") else []
            entry = CognitionEntry(
                content=documents[i] if i < len(documents) else "",
                source=meta.get("source", ""),
                entry_type=meta.get("entry_type", "unknown"),
                tags=tags,
                entry_id=ids[i],
            )
            entries.append(entry)
        return entries

    def retrieve_by_tags(self, tags: list[str], n_results: int = 10) -> list[CognitionEntry]:
        all_entries: list[tuple[CognitionEntry, str]] = []
        for entry_id, entry in self._entries.items():
            if any(t in entry.tags for t in tags):
                all_entries.append((entry, entry.content))
        if not all_entries:
            return []
        results = sorted(all_entries, key=lambda x: len(set(x[0].tags) & set(tags)), reverse=True)
        return [e[0] for e in results[:n_results]]

    def count(self) -> int:
        return self.collection.count()

    def get_all_entries(self) -> list[CognitionEntry]:
        return list(self._entries.values())

    def persist(self) -> None:
        logger.info(f"CognitionBase: {self.count()} entrees persiste es.")

    def _embed(self, text: str) -> Any:
        model = self._load_model()
        return model.encode(text, show_progress_bar=False, convert_to_numpy=True)
