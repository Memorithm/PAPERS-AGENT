"""
Embedding Service - adapte de l'implementation ASI-Evolve.
"""

from __future__ import annotations

import numpy as np

try:
    from sentence_transformers import SentenceTransformer
    HAS_ST = True
except ImportError:
    HAS_ST = False


class EmbeddingService:
    def __init__(
        self,
        model_name: str = "sentence-transformers/all-MiniLM-L6-v2",
        device: str = "cpu",
    ) -> None:
        if not HAS_ST:
            raise ImportError("sentence-transformers non installe.")
        self.model = SentenceTransformer(model_name, device=device)
        self.dimension = self.model.get_sentence_embedding_dimension()

    def encode(self, texts: str | list[str], normalize: bool = True) -> np.ndarray:
        if isinstance(texts, str):
            texts = [texts]
        embeddings = self.model.encode(
            texts,
            normalize_embeddings=normalize,
            show_progress_bar=False,
        )
        return np.array(embeddings, dtype=np.float32)

    def get_dimension(self) -> int:
        return self.dimension
