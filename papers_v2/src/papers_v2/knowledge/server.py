from __future__ import annotations

from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException
from fastapi.responses import HTMLResponse
from loguru import logger

from papers_v2.knowledge.graph import KnowledgeGraph

app = FastAPI(title="PAPERS V2 Knowledge Graph")


@app.get("/", response_class=HTMLResponse)
def index() -> str:
    return """\n    <html>\n        <head>\u003ctitle>PAPERS V2 Knowledge</title>\u003c/head>\n        <body>\n            <h1>PAPERS V2 Knowledge Graph</h1>\n            <ul>\n                <li><a href="/api/papers">Papiers</a></li>\n                <li><a href="/api/stats">Statistiques</a></li>\n            </ul>\n        </body>\n    </html>\n    """


@app.get("/api/papers")
def list_papers() -> list[dict[str, Any]]:
    kg = KnowledgeGraph()
    papers = [
        {"id": node, **attrs}
        for node, attrs in kg.graph.nodes(data=True)
        if attrs.get("kind") == "paper"
    ]
    return papers


@app.get("/api/papers/{paper_id}")
def get_paper(paper_id: str) -> dict[str, Any]:
    kg = KnowledgeGraph()
    node = f"paper:{paper_id}"
    if node not in kg.graph:
        raise HTTPException(status_code=404, detail="Papier non trouvé")
    attrs = dict(kg.graph.nodes[node])
    neighbors = [
        {"id": n, **kg.graph.nodes[n], "relation": attrs.get("label", "related")}
        for n in kg.graph.neighbors(node)
    ]
    return {"paper": attrs, "neighbors": neighbors}


@app.get("/api/stats")
def stats() -> dict[str, Any]:
    kg = KnowledgeGraph()
    return {
        "total_nodes": kg.graph.number_of_nodes(),
        "total_edges": kg.graph.number_of_edges(),
        "papers": sum(1 for _, a in kg.graph.nodes(data=True) if a.get("kind") == "paper"),
        "contributions": sum(1 for _, a in kg.graph.nodes(data=True) if a.get("kind") == "contribution"),
        "modules": sum(1 for _, a in kg.graph.nodes(data=True) if a.get("kind") == "module"),
    }


def serve(host: str, port: int, knowledge_graph_path: str | None = None) -> None:
    import uvicorn
    logger.info(f"Démarrage du serveur PAPERS V2 sur {host}:{port}")
    uvicorn.run(app, host=host, port=port)
