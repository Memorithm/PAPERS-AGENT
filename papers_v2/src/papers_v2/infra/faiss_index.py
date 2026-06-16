"""
FAISS Index - adapté de l'implementation ASI-Evolve.
Plus rapide que ChromaDB pour la recherche de similarité.
"""

from __future__ import annotations

import pickle
from pathlib import Path
from threading import RLock
from typing import Any

import numpy as np

try:
    import faiss
    HAS_FAISS = True
except ImportError:
    HAS_FAISS = False


class FAISSIndex:
    def __init__(
        self,
        dimension: int = 384,
        index_type: str = "IP",
        storage_path: str | Path | None = None,
    ) -> None:
        if not HAS_FAISS:
            raise ImportError("FAISS non installe. Run: pip install faiss-cpu")

        self.dimension = dimension
        self.index_type = index_type
        self.storage_path = Path(storage_path) if storage_path else None
        self.lock = RLock()

        if index_type == "IP":
            self.index = faiss.IndexFlatIP(dimension)
        else:
            self.index = faiss.IndexFlatL2(dimension)

        self.id_to_idx: dict[int, int] = {}
        self.idx_to_id: dict[int, int] = {}
        self.next_idx = 0

        if self.storage_path:
            self._load()

    def add(self, node_id: int, vector: np.ndarray) -> None:
        with self.lock:
            if node_id in self.id_to_idx:
                return
            vector = self._normalize(vector.reshape(1, -1).astype(np.float32))
            self.index.add(vector)
            self.id_to_idx[node_id] = self.next_idx
            self.idx_to_id[self.next_idx] = node_id
            self.next_idx += 1

    def search(
        self, query_vector: np.ndarray, top_k: int = 5, score_threshold: float = 0.0,
    ) -> list[tuple[int, float]]:
        with self.lock:
            if self.index.ntotal == 0:
                return []
            query_vector = self._normalize(query_vector.reshape(1, -1).astype(np.float32))
            k = min(top_k, self.index.ntotal)
            scores, indices = self.index.search(query_vector, k)
            results: list[tuple[int, float]] = []
            for score, idx in zip(scores[0], indices[0]):
                if idx < 0 or score < score_threshold:
                    continue
                node_id = self.idx_to_id.get(int(idx))
                if node_id is not None:
                    results.append((node_id, float(score)))
            return results

    def remove(self, node_id: int) -> None:
        with self.lock:
            if node_id in self.id_to_idx:
                idx = self.id_to_idx.pop(node_id)
                self.idx_to_id.pop(idx, None)

    def _normalize(self, vectors: np.ndarray) -> np.ndarray:
        if self.index_type == "IP":
            norms = np.linalg.norm(vectors, axis=1, keepdims=True)
            norms[norms == 0] = 1
            return vectors / norms
        return vectors

    def save(self) -> None:
        if not self.storage_path or not HAS_FAISS:
            return
        with self.lock:
            self.storage_path.mkdir(parents=True, exist_ok=True)
            faiss.write_index(self.index, str(self.storage_path / "faiss.index"))
            with open(self.storage_path / "faiss_meta.pkl", "wb") as f:
                pickle.dump({
                    "id_to_idx": self.id_to_idx,
                    "idx_to_id": self.idx_to_id,
                    "next_idx": self.next_idx,
                }, f)

    def _load(self) -> None:
        if not self.storage_path or not HAS_FAISS:
            return
        index_file = self.storage_path / "faiss.index"
        meta_file = self.storage_path / "faiss_meta.pkl"
        if not index_file.exists() or not meta_file.exists():
            return
        with self.lock:
            self.index = faiss.read_index(str(index_file))
            with open(meta_file, "rb") as f:
                meta = pickle.load(f)
            self.id_to_idx = meta["id_to_idx"]
            self.idx_to_id = meta["idx_to_id"]
            self.next_idx = meta["next_idx"]

    def reset(self) -> None:
        with self.lock:
            if HAS_FAISS:
                if self.index_type == "IP":
                    self.index = faiss.IndexFlatIP(self.dimension)
                else:
                    self.index = faiss.IndexFlatL2(self.dimension)
            self.id_to_idx.clear()
            self.idx_to_id.clear()
            self.next_idx = 0

    @property
    def size(self) -> int:
        return len(self.id_to_idx)
