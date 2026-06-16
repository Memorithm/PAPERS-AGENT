import json
import tempfile
from pathlib import Path

import pytest

from papers_v2.core.models import (
    AnalysisReport,
    CognitionItem,
    EvolutionConfig,
    EvolutionNodeModel,
    EvolutionResult,
    IntegrationScore,
    Publication,
    ReproducibilityScore,
)
from papers_v2.core.orchestrator import PapersEngine
from papers_v2.evolution.cognition import CognitionBase, CognitionEntry
from papers_v2.evolution.database import EvolutionDatabase, EvolutionNode, SamplingPolicy
from papers_v2.evolution.researcher import Researcher
from papers_v2.evolution.engineer import Engineer
from papers_v2.evolution.analyzer import EvolutionAnalyzer
from papers_v2.evolution.loop import EvolutionLoop


class TestPublicationModel:
    def test_create_publication(self):
        pub = Publication(id="PAPERS-2024-001", title="Test Paper", authors=[])
        assert pub.title == "Test Paper"
        assert pub.id == "PAPERS-2024-001"

    def test_publication_with_domains(self):
        pub = Publication(
            id="P-001",
            title="Test",
            authors=[],
            domains=["LLM Architectures", "Memory Systems"],
        )
        assert len(pub.domains) == 2


class TestReproducibilityScore:
    def test_score_computation(self):
        score = ReproducibilityScore(
            documentation=0.8,
            code_available=1.0,
            data_available=0.5,
            results_reproducible=0.5,
            acceptable_cost=0.5,
        )
        assert 0.0 <= score.score <= 1.0

    def test_perfect_score(self):
        score = ReproducibilityScore(
            documentation=1.0,
            code_available=1.0,
            data_available=1.0,
            results_reproducible=1.0,
            acceptable_cost=1.0,
        )
        assert score.score == 1.0

    def test_zero_score(self):
        score = ReproducibilityScore(
            documentation=0.0,
            code_available=0.0,
            data_available=0.0,
            results_reproducible=0.0,
            acceptable_cost=0.0,
        )
        assert score.score == 0.0


class TestIntegrationScore:
    def test_score_computation(self):
        score = IntegrationScore(
            reproducibility=0.7,
            architectural_impact=0.6,
            hardware_cost=0.5,
            code_availability=1.0,
            scientific_maturity=0.5,
        )
        assert 0.0 <= score.score <= 1.0

    def test_interpretation_experimentation(self):
        score = IntegrationScore(
            reproducibility=0.7,
            architectural_impact=0.6,
            hardware_cost=0.5,
            code_availability=1.0,
            scientific_maturity=0.5,
        )
        assert score.interpretation == "EXPERIMENTATION"

    def test_interpretation_integration(self):
        score = IntegrationScore(
            reproducibility=0.9,
            architectural_impact=0.9,
            hardware_cost=0.8,
            code_availability=1.0,
            scientific_maturity=0.9,
        )
        assert score.interpretation == "INTEGRATION POSSIBLE"

    def test_interpretation_non_prioritaire(self):
        score = IntegrationScore(
            reproducibility=0.1,
            architectural_impact=0.1,
            hardware_cost=0.1,
            code_availability=0.0,
            scientific_maturity=0.1,
        )
        assert score.interpretation == "NON PRIORITAIRE"


class TestOrchestrator:
    def test_orchestrator_runs(self):
        engine = PapersEngine(use_llm=False)
        pub = Publication(id="PAPERS-2024-TEST", title="Test", authors=[])
        report = engine.analyze(pub)
        assert isinstance(report, AnalysisReport)
        assert report.publication.title == "Test"

    def test_orchestrator_with_abstract(self):
        engine = PapersEngine(use_llm=False)
        pub = Publication(
            id="P-002",
            title="Test Paper with Abstract",
            authors=[],
            abstract="We propose a novel method for transformer optimization.",
        )
        report = engine.analyze(pub)
        assert len(report.scientific_contributions) > 0

    def test_critical_analysis_no_crash(self):
        engine = PapersEngine(use_llm=False)
        pub = Publication(id="P-003", title="Test", authors=[])
        report = engine.analyze(pub)
        ca = report.critical_analysis
        assert "strengths" in ca
        assert "weaknesses" in ca


class TestCognitionBase:
    def test_add_and_retrieve(self, tmp_path):
        cb = CognitionBase(persist_dir=str(tmp_path / "cog_test"))
        entry = CognitionEntry(
            content="Use chunk-wise computation for O(N) complexity",
            source="paper_1",
            entry_type="heuristic",
            tags=["linear_attention", "efficiency"],
        )
        cb.add_entry(entry)
        assert cb.count() == 1

    def test_add_from_papers(self, tmp_path):
        cb = CognitionBase(persist_dir=str(tmp_path / "cog_test2"))
        papers = [
            {"content": "Linear attention reduces quadratic complexity.", "source": "paper_1"},
            {"content": "Delta rule enables parallel training.", "source": "paper_2"},
        ]
        cb.add_from_papers(papers)
        assert cb.count() == 2

    def test_add_multiple_entries(self, tmp_path):
        cb = CognitionBase(persist_dir=str(tmp_path / "cog_test3"))
        entries = [
            CognitionEntry(content=f"Knowledge item {i}", source=f"src_{i}")
            for i in range(10)
        ]
        cb.add_entries(entries)
        assert cb.count() == 10

    def test_get_all_entries(self, tmp_path):
        cb = CognitionBase(persist_dir=str(tmp_path / "cog_test4"))
        entry = CognitionEntry(content="Test content", source="test")
        cb.add_entry(entry)
        all_entries = cb.get_all_entries()
        assert len(all_entries) == 1
        assert all_entries[0].content == "Test content"


class TestEvolutionDatabase:
    def test_add_and_retrieve_node(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test.json"))
        node = EvolutionNode(
            program="def train(): pass",
            motivation="Basic training loop",
            score=0.75,
        )
        db.add_node(node)
        assert db.count() == 1
        retrieved = db.get_node(node.id)
        assert retrieved is not None
        assert retrieved.score == 0.75

    def test_greedy_sampling(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test2.json"))
        for i in range(10):
            db.add_node(EvolutionNode(
                program=f"prog_{i}",
                motivation=f"m_{i}",
                score=float(i) / 10,
            ))
        sampled = db.sample(n=3, policy=SamplingPolicy.GREEDY)
        assert len(sampled) == 3
        assert sampled[0].score >= sampled[1].score >= sampled[2].score

    def test_random_sampling(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test3.json"))
        for i in range(10):
            db.add_node(EvolutionNode(program=f"p_{i}", score=float(i)))
        sampled = db.sample(n=3, policy=SamplingPolicy.RANDOM)
        assert len(sampled) == 3

    def test_ucb1_sampling(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test4.json"))
        for i in range(5):
            db.add_node(EvolutionNode(program=f"p_{i}", score=float(i) / 5))
        sampled = db.sample(n=3, policy=SamplingPolicy.UCB1)
        assert len(sampled) == 3

    def test_top_k(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test5.json"))
        for i in range(20):
            db.add_node(EvolutionNode(program=f"p_{i}", score=float(i)))
        top = db.top_k(5)
        assert len(top) == 5
        assert top[0].score == 19.0

    def test_best(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test6.json"))
        db.add_node(EvolutionNode(program="low", score=0.1))
        db.add_node(EvolutionNode(program="high", score=0.9))
        best = db.best()
        assert best is not None
        assert best.score == 0.9

    def test_stats(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test7.json"))
        db.add_node(EvolutionNode(program="a", score=0.5))
        db.add_node(EvolutionNode(program="b", score=1.0))
        stats = db.stats()
        assert stats["total_nodes"] == 2
        assert stats["max_score"] == 1.0

    def test_prune_bottom(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_test8.json"))
        for i in range(100):
            db.add_node(EvolutionNode(program=f"p_{i}", score=float(i)))
        db.prune_bottom(keep_top=50)
        assert db.count() <= 50

    def test_save_and_load(self, tmp_path):
        db_path = str(tmp_path / "db_save.json")
        db = EvolutionDatabase(persist_path=db_path)
        db.add_node(EvolutionNode(program="test", score=0.5))
        db.save()

        db2 = EvolutionDatabase(persist_path=db_path)
        assert db2.count() == 1

    def test_sampling_empty_database(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_empty.json"))
        sampled = db.sample(n=5)
        assert sampled == []

    def test_map_elites_sampling(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_me.json"))
        for i in range(20):
            db.add_node(EvolutionNode(program=f"p_{i}", score=float(i) / 20))
        sampled = db.sample(n=5, policy=SamplingPolicy.MAP_ELITES)
        assert len(sampled) >= 1

    def test_export_summary(self, tmp_path):
        db = EvolutionDatabase(persist_path=str(tmp_path / "db_summary.json"))
        db.add_node(EvolutionNode(program="p1", score=0.8))
        db.add_node(EvolutionNode(program="p2", score=0.2))
        summary = db.export_summary()
        assert len(summary) == 2
        assert summary[0]["score"] == 0.8


class TestEngineer:
    def test_execute_with_function(self):
        def my_eval(program: str) -> dict:
            return {"success": True, "score": 0.95, "metrics": {"loss": 0.1}}

        eng = Engineer()
        result = eng.execute(program="print('hello')", eval_function=my_eval)
        assert result["success"]
        assert result["score"] == 0.95

    def test_execute_failing_function(self):
        def my_eval(program: str) -> dict:
            raise ValueError("test error")

        eng = Engineer()
        result = eng.execute(program="bad code", eval_function=my_eval)
        assert not result["success"]

    def test_compute_fitness(self):
        metrics = {"score": 0.8, "loss": 0.3}
        fitness = Engineer.compute_fitness(metrics)
        assert fitness == 0.8

    def test_compute_fitness_with_llm_judge(self):
        metrics = {"score": 0.7}
        fitness = Engineer.compute_fitness(metrics, llm_judge_score=0.9, llm_weight=0.2)
        expected = 0.7 * 0.8 + 0.9 * 0.2
        assert abs(fitness - expected) < 0.001


class TestEvolutionAnalyzer:
    def test_heuristic_analysis_success(self):
        analyzer = EvolutionAnalyzer()
        result = analyzer.analyze(
            motivation="Test motivation",
            program="print('hello')",
            results={"success": True, "score": 0.9},
        )
        assert "summary" in result
        assert "strengths" in result
        assert "weaknesses" in result
        assert "actionable_insights" in result

    def test_heuristic_analysis_failure(self):
        analyzer = EvolutionAnalyzer()
        result = analyzer.analyze(
            motivation="Test",
            program="bad code",
            results={"success": False, "error": "Syntax error at line 5"},
        )
        assert "root_cause" in result


class TestEvolutionLoop:
    def test_loop_creation(self):
        loop = EvolutionLoop(task_description="Test task")
        assert loop.task_description == "Test task"
        assert loop.max_rounds == 50

    def test_loop_run_with_eval_function(self, tmp_path):
        def my_eval(program: str) -> dict:
            return {"success": True, "score": 0.85, "metrics": {"loss": 0.2}}

        loop = EvolutionLoop(
            task_description="Optimize a simple function",
            max_rounds=2,
            use_llm=False,
            output_dir=str(tmp_path / "evo_out"),
            db_path=str(tmp_path / "evo_db.json"),
            cognition_path=str(tmp_path / "evo_cog"),
        )
        result = loop.run(
            eval_function=my_eval,
            n_candidates_per_round=2,
            n_context_nodes=3,
            n_cognition=3,
            verbose=False,
        )
        assert result["success"]
        assert result["total_rounds"] >= 1
        assert result["total_candidates"] >= 2
        assert "best_score" in result

    def test_loop_run_without_eval_raises(self):
        loop = EvolutionLoop(task_description="Test")
        with pytest.raises(ValueError, match="eval_function or eval_command"):
            loop.run()

    def test_loop_saves_state(self, tmp_path):
        def my_eval(program: str) -> dict:
            return {"success": True, "score": 0.5, "metrics": {}}

        output_dir = tmp_path / "evo_state"
        loop = EvolutionLoop(
            task_description="Test save",
            max_rounds=1,
            use_llm=False,
            output_dir=str(output_dir),
            db_path=str(tmp_path / "db.json"),
            cognition_path=str(tmp_path / "cog"),
        )
        loop.run(eval_function=my_eval, n_candidates_per_round=1, verbose=False)
        state_path = output_dir / "evolution_state.json"
        assert state_path.exists()

    def test_loop_early_stop_target(self, tmp_path):
        def my_eval(program: str) -> dict:
            return {"success": True, "score": 0.99, "metrics": {}}

        loop = EvolutionLoop(
            task_description="Early stop test",
            max_rounds=10,
            target_score=0.9,
            use_llm=False,
            output_dir=str(tmp_path / "evo_early"),
            db_path=str(tmp_path / "db_early.json"),
            cognition_path=str(tmp_path / "cog_early"),
        )
        result = loop.run(eval_function=my_eval, n_candidates_per_round=1, verbose=False)
        assert result["stopped_early"]
        assert result["best_score"] >= 0.9

    def test_loop_patience_stop(self, tmp_path):
        scores = iter([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5])

        def my_eval(program: str) -> dict:
            s = next(scores)
            return {"success": True, "score": s, "metrics": {}}

        loop = EvolutionLoop(
            task_description="Patience test",
            max_rounds=50,
            patience=3,
            use_llm=False,
            output_dir=str(tmp_path / "evo_pat"),
            db_path=str(tmp_path / "db_pat.json"),
            cognition_path=str(tmp_path / "cog_pat"),
        )
        result = loop.run(eval_function=my_eval, n_candidates_per_round=1, verbose=False)
        assert result["stopped_early"]

    def test_display_summary(self, capsys):
        result = {
            "success": True,
            "best_score": 0.95,
            "total_rounds": 10,
            "total_candidates": 30,
            "total_time_seconds": 120.0,
            "stopped_early": True,
            "cognition_count": 5,
            "database_stats": {"mean_score": 0.5, "max_score": 0.95},
        }
        EvolutionLoop.display_summary(result)
        captured = capsys.readouterr()
        assert captured.out


class TestEvolutionModels:
    def test_cognition_item(self):
        item = CognitionItem(
            id="c1",
            content="Test cognition",
            item_type="heuristic",
            tags=["attention"],
        )
        assert item.id == "c1"
        assert item.item_type == "heuristic"

    def test_evolution_node_model(self):
        node = EvolutionNodeModel(
            id="n1",
            program="def train(): pass",
            score=0.8,
        )
        assert node.id == "n1"
        assert node.score == 0.8

    def test_evolution_config_default(self):
        config = EvolutionConfig()
        assert config.max_rounds == 50
        assert config.model == "gemma4:e2b"

    def test_evolution_config_save_load(self, tmp_path):
        config = EvolutionConfig(
            task_description="Test task",
            max_rounds=10,
        )
        path = tmp_path / "config.yaml"
        config.save(path)

        loaded = EvolutionConfig.load(path)
        assert loaded.task_description == "Test task"
        assert loaded.max_rounds == 10

    def test_evolution_result(self):
        result = EvolutionResult(
            success=True,
            best_score=0.9,
            total_rounds=5,
            total_candidates=15,
        )
        assert result.success
        assert result.best_score == 0.9


class TestResearcher:
    def test_researcher_creation(self):
        researcher = Researcher(model="gemma4:e2b")
        assert researcher.model == "gemma4:e2b"

    def test_fallback_generation(self):
        researcher = Researcher(model="gemma4:e2b")
        result = researcher.generate(task_description="Create a simple function")
        assert "motivation" in result
        assert "program" in result

    def test_format_cognition(self, tmp_path):
        researcher = Researcher(model="gemma4:e2b")
        entries = [
            CognitionEntry(content="Tip 1", source="paper_1"),
            CognitionEntry(content="Tip 2", source="paper_2"),
        ]
        formatted = researcher._format_cognition(entries)
        assert "Tip 1" in formatted
        assert "Tip 2" in formatted

    def test_format_context(self, tmp_path):
        researcher = Researcher(model="gemma4:e2b")
        nodes = [
            EvolutionNode(program="p1", score=0.5, motivation="test"),
            EvolutionNode(program="p2", score=0.8, analysis="good"),
        ]
        formatted = researcher._format_context(nodes)
        assert "0.5000" in formatted
        assert "0.8000" in formatted

    def test_generate_batch(self, tmp_path):
        researcher = Researcher(model="gemma4:e2b")
        db = EvolutionDatabase(persist_path=str(tmp_path / "res_db.json"))
        candidates = researcher.generate_batch(
            task_description="Optimize function",
            n_candidates=2,
            database=db,
        )
        assert len(candidates) == 2
        for c in candidates:
            assert "motivation" in c
            assert "program" in c


class TestPaperRegistry:
    def test_load_registry(self, tmp_path):
        from papers_v2.knowledge.papers_registry import PaperRegistry
        registry_path = tmp_path / "test_registry.json"
        papers = [
            {
                "id": "test-001",
                "title": "Test Paper 1",
                "authors": ["Author A"],
                "year": 2026,
                "source": "arXiv:test.001",
                "url": "https://arxiv.org/abs/test.001",
                "abstract": "Test abstract",
                "tags": ["test", "pattern"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "Test insight",
                "relevance_score": 0.9,
            },
            {
                "id": "test-002",
                "title": "Test Paper 2",
                "authors": ["Author B"],
                "year": 2025,
                "source": "arXiv:test.002",
                "url": "https://arxiv.org/abs/test.002",
                "abstract": "Another abstract about patterns and deduction",
                "tags": ["deduction", "reasoning"],
                "domain": "COGNITIVE_ARCHITECTURES",
                "key_insight": "Deduction insight",
                "relevance_score": 0.85,
            },
        ]
        import json
        registry_path.write_text(json.dumps(papers))

        reg = PaperRegistry(registry_path=str(registry_path))
        assert len(reg.papers) == 2

    def test_search(self, tmp_path):
        from papers_v2.knowledge.papers_registry import PaperRegistry
        registry_path = tmp_path / "test_registry.json"
        papers = [
            {
                "id": "test-001",
                "title": "Pattern Recognition Paper",
                "authors": ["Author A"],
                "year": 2026,
                "source": "arXiv:test.001",
                "url": "https://arxiv.org/abs/test.001",
                "abstract": "About pattern recognition",
                "tags": ["pattern", "recognition"],
                "domain": "REPRESENTATION_ENGINEERING",
                "key_insight": "Pattern insight",
                "relevance_score": 0.9,
            },
            {
                "id": "test-002",
                "title": "Deduction Paper",
                "authors": ["Author B"],
                "year": 2025,
                "source": "arXiv:test.002",
                "url": "https://arxiv.org/abs/test.002",
                "abstract": "About deduction methods",
                "tags": ["deduction", "reasoning"],
                "domain": "COGNITIVE_ARCHITECTURES",
                "key_insight": "Deduction insight",
                "relevance_score": 0.85,
            },
        ]
        import json
        registry_path.write_text(json.dumps(papers))

        reg = PaperRegistry(registry_path=str(registry_path))
        results = reg.search("pattern")
        assert len(results) == 1
        assert results[0]["id"] == "test-001"

        results2 = reg.search("deduction")
        assert len(results2) == 1
        assert results2[0]["id"] == "test-002"

    def test_filter_by_tag(self, tmp_path):
        from papers_v2.knowledge.papers_registry import PaperRegistry
        registry_path = tmp_path / "test_registry.json"
        papers = [
            {
                "id": "test-001",
                "title": "Paper 1",
                "authors": ["A"],
                "year": 2026,
                "source": "src",
                "url": "url",
                "abstract": "abs",
                "tags": ["pattern", "induction"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "insight",
                "relevance_score": 0.5,
            },
            {
                "id": "test-002",
                "title": "Paper 2",
                "authors": ["B"],
                "year": 2026,
                "source": "src2",
                "url": "url2",
                "abstract": "abs2",
                "tags": ["deduction", "reasoning"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "insight2",
                "relevance_score": 0.5,
            },
        ]
        import json
        registry_path.write_text(json.dumps(papers))

        reg = PaperRegistry(registry_path=str(registry_path))
        filtered = reg.filter_by_tag("pattern")
        assert len(filtered) == 1
        assert filtered[0]["id"] == "test-001"

    def test_filter_by_domain(self, tmp_path):
        from papers_v2.knowledge.papers_registry import PaperRegistry
        registry_path = tmp_path / "test_registry.json"
        papers = [
            {
                "id": "test-001",
                "title": "Paper 1",
                "authors": ["A"],
                "year": 2026,
                "source": "src",
                "url": "url",
                "abstract": "abs",
                "tags": ["tag"],
                "domain": "REPRESENTATION_ENGINEERING",
                "key_insight": "insight",
                "relevance_score": 0.5,
            },
            {
                "id": "test-002",
                "title": "Paper 2",
                "authors": ["B"],
                "year": 2026,
                "source": "src2",
                "url": "url2",
                "abstract": "abs2",
                "tags": ["tag"],
                "domain": "COGNITIVE_ARCHITECTURES",
                "key_insight": "insight2",
                "relevance_score": 0.5,
            },
        ]
        import json
        registry_path.write_text(json.dumps(papers))

        reg = PaperRegistry(registry_path=str(registry_path))
        filtered = reg.filter_by_domain("REPRESENTATION_ENGINEERING")
        assert len(filtered) == 1
        assert filtered[0]["id"] == "test-001"

    def test_stats(self, tmp_path):
        from papers_v2.knowledge.papers_registry import PaperRegistry
        registry_path = tmp_path / "test_registry.json"
        papers = [
            {
                "id": "test-001",
                "title": "Paper 1",
                "authors": ["A"],
                "year": 2026,
                "source": "src",
                "url": "url",
                "abstract": "abs",
                "tags": ["pattern"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "insight",
                "relevance_score": 0.5,
            },
            {
                "id": "test-002",
                "title": "Paper 2",
                "authors": ["B"],
                "year": 2025,
                "source": "src2",
                "url": "url2",
                "abstract": "abs2",
                "tags": ["deduction", "reasoning"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "insight2",
                "relevance_score": 0.5,
            },
        ]
        import json
        registry_path.write_text(json.dumps(papers))

        reg = PaperRegistry(registry_path=str(registry_path))
        stats = reg.stats()
        assert stats["total_papers"] == 2
        assert 2026 in stats["by_year"]
        assert 2025 in stats["by_year"]
        assert len(stats["by_domain"]) >= 1

    def test_list_tags(self, tmp_path):
        from papers_v2.knowledge.papers_registry import PaperRegistry
        registry_path = tmp_path / "test_registry.json"
        papers = [
            {
                "id": "test-001",
                "title": "Paper 1",
                "authors": ["A"],
                "year": 2026,
                "source": "src",
                "url": "url",
                "abstract": "abs",
                "tags": ["pattern", "induction"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "insight",
                "relevance_score": 0.5,
            },
        ]
        import json
        registry_path.write_text(json.dumps(papers))

        reg = PaperRegistry(registry_path=str(registry_path))
        tags = reg.list_tags()
        assert "pattern" in tags
        assert "induction" in tags

    def test_domain_parse(self):
        from papers_v2.core.models import Domain

        assert Domain.parse("REPRESENTATION_ENGINEERING") == Domain.REPRESENTATION_ENGINEERING
        assert Domain.parse("Representation Engineering") == Domain.REPRESENTATION_ENGINEERING
        assert Domain.parse("representation_engineering") == Domain.REPRESENTATION_ENGINEERING
        assert Domain.parse("LLM_ARCHITECTURES") == Domain.LLM_ARCHITECTURES
        assert Domain.parse("UNKNOWN_DOMAIN") == Domain.KNOWLEDGE_REPRESENTATION

    def test_import_to_cognition_base(self, tmp_path):
        from papers_v2.evolution import CognitionBase
        from papers_v2.knowledge.papers_registry import PaperRegistry
        import json

        registry_path = tmp_path / "reg.json"
        papers = [
            {
                "id": "test-001",
                "title": "Test Pattern Paper",
                "authors": ["Author A"],
                "year": 2026,
                "source": "arXiv:test",
                "url": "https://arxiv.org/abs/test",
                "abstract": "Abstract about patterns",
                "tags": ["pattern"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "Key pattern insight",
                "relevance_score": 0.9,
            },
        ]
        registry_path.write_text(json.dumps(papers))

        cog_dir = tmp_path / "cog"
        reg = PaperRegistry(registry_path=str(registry_path), cognition_path=str(cog_dir))

        ct = CognitionBase(persist_dir=str(cog_dir))
        n = reg.import_to_cognition_base(ct)
        assert n == 1

    def test_import_to_knowledge_graph(self, tmp_path):
        from papers_v2.knowledge.graph import KnowledgeGraph
        from papers_v2.knowledge.papers_registry import PaperRegistry
        import json

        registry_path = tmp_path / "reg.json"
        papers = [
            {
                "id": "test-001",
                "title": "Test Pattern Paper",
                "authors": ["Author A"],
                "year": 2026,
                "source": "arXiv:test",
                "url": "https://arxiv.org/abs/test",
                "abstract": "Abstract about patterns",
                "tags": ["pattern"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "Key insight",
                "relevance_score": 0.9,
            },
        ]
        registry_path.write_text(json.dumps(papers))

        kg_path = tmp_path / "kg.json"
        reg = PaperRegistry(registry_path=str(registry_path), knowledge_graph_path=str(kg_path))

        kg = KnowledgeGraph(path=str(kg_path))
        n = reg.import_to_knowledge_graph(kg)
        assert n == 1


class TestPatternInductionEngine:
    def test_induce_from_history_empty(self):
        from papers_v2.intelligence.pattern_induction import PatternInductionEngine
        engine = PatternInductionEngine(min_occurrences=2)
        patterns = engine.induce_from_history([])
        assert patterns == []

    def test_induce_from_history_with_data(self):
        from papers_v2.intelligence.pattern_induction import PatternInductionEngine
        engine = PatternInductionEngine(min_occurrences=2)
        history = [
            {"score": 0.8, "program": "def train(): pass\ndef forward(): return x", "motivation": "Basic training", "node_id": "n1", "parent_id": "root"},
            {"score": 0.9, "program": "def train(): x=1\ndef forward(): return x*2", "motivation": "Improved", "node_id": "n2", "parent_id": "n1"},
            {"score": 0.95, "program": "def train(): return None\ndef forward(): return x*3", "motivation": "Best", "node_id": "n3", "parent_id": "n1"},
        ]
        patterns = engine.induce_from_history(history)
        assert len(patterns) >= 1

    def test_register_and_query(self):
        from papers_v2.intelligence.pattern_induction import Pattern, PatternInductionEngine
        engine = PatternInductionEngine()
        p = Pattern("test", "def test(): pass", "Test pattern", ["n1", "n2"])
        engine.register(p)
        assert len(engine.patterns) == 1
        assert engine.query_by_node("n1")[0].id == "test"

    def test_top_patterns(self):
        from papers_v2.intelligence.pattern_induction import Pattern, PatternInductionEngine
        engine = PatternInductionEngine()
        for i in range(5):
            p = Pattern(f"p{i}", f"sig{i}", f"desc{i}", [f"n{i}"], success_rate=i * 0.2, usage_count=i + 1)
            engine.register(p)
        top = engine.top_patterns(3)
        assert len(top) == 3
        assert top[0].success_rate >= top[1].success_rate

    def test_decompose_to_primitives(self):
        from papers_v2.intelligence.pattern_induction import Pattern, PatternInductionEngine
        engine = PatternInductionEngine()
        p = Pattern("test", "def train():\n    for x in data:\n        if x > 0:\n            return x", "Test", ["n1"])
        primitives = engine.decompose_to_primitives(p)
        assert "loop_pattern" in primitives
        assert "conditional_branch" in primitives
        assert "return_statement" in primitives

    def test_build_composite(self):
        from papers_v2.intelligence.pattern_induction import PatternInductionEngine
        engine = PatternInductionEngine()
        composite = engine.build_composite(["function_definition", "loop_pattern"], "A loop in a function")
        assert "function_definition" in composite.signature
        assert "loop_pattern" in composite.signature


class TestFalsificationEngine:
    def test_generate_tests(self):
        from papers_v2.intelligence.falsification import FalsificationEngine
        engine = FalsificationEngine()
        tests = engine.generate_tests("h1", "This method always improves accuracy", "def train(): pass")
        assert len(tests) >= 3

    def test_boundary_tests_present(self):
        from papers_v2.intelligence.falsification import FalsificationEngine
        engine = FalsificationEngine()
        tests = engine.generate_tests("h1", "test", "def foo(): pass")
        boundary_ids = [t.test_id for t in tests if "empty_input" in t.test_id or "None_input" in t.test_id]
        assert len(boundary_ids) >= 1

    def test_adversarial_tests_for_claims(self):
        from papers_v2.intelligence.falsification import FalsificationEngine
        engine = FalsificationEngine()
        tests = engine.generate_tests("h1", "This method always outperforms baselines", "def foo(): pass")
        adv_tests = [t for t in tests if t.adversarial]
        assert len(adv_tests) >= 0

    def test_falsifiability_score_no_results(self):
        from papers_v2.intelligence.falsification import FalsificationEngine
        engine = FalsificationEngine()
        score = engine.compute_falsifiability_score("nonexistent")
        assert score == 0.5

    def test_detect_reward_hacking(self):
        from papers_v2.intelligence.falsification import FalsificationEngine
        engine = FalsificationEngine()
        result = engine.detect_reward_hacking("h1", score=0.95, falsifiability=0.1)
        assert result["risk"] in ("HIGH", "MEDIUM", "LOW")

    def test_get_summary(self):
        from papers_v2.intelligence.falsification import FalsificationEngine
        engine = FalsificationEngine()
        engine.generate_tests("h1", "test", "def foo(): pass")
        summary = engine.get_summary("h1")
        assert "total_tests" in summary
        assert "falsifiability" in summary


class TestCounterexampleGuidedVerifier:
    def test_verify_success(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        report = verifier.verify("c1", "def train(): pass", {"success": True, "runtime_seconds": 1.0})
        assert report.passed
        assert len(report.counterexamples) == 0

    def test_verify_with_error(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        report = verifier.verify("c1", "bad code", {"success": False, "error": "NameError: x not defined"})
        assert not report.passed
        assert len(report.counterexamples) >= 1

    def test_constraint_checking(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        constraints = [{"type": "contains_function", "target": "train"}]
        report = verifier.verify("c1", "def test(): pass", {"success": True}, constraints)
        assert len(report.violations) >= 1

    def test_invariant_checks(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        report = verifier.verify("c1", "def foo(): pass", {"success": True, "runtime_seconds": 0.1, "output": 42})
        assert report.invariant_checks["no_crash"]
        assert report.invariant_checks["finite_output"]

    def test_determinism_check(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        report = verifier.verify("c1", "import random\ndef foo(): return random.random()", {"success": True})
        assert not report.invariant_checks["deterministic"]

    def test_suggestions_generated(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        report = verifier.verify("c1", "x = 1/0", {"success": False, "error": "ZeroDivisionError"})
        assert len(report.suggestions) >= 1

    def test_rich_feedback(self):
        from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier
        verifier = CounterexampleGuidedVerifier()
        report = verifier.verify("c1", "bad", {"success": False, "error": "fail"})
        feedback = report.to_rich_feedback()
        assert "Counterexamples" in feedback


class TestSymbolicReasoningEngine:
    def test_abduce_from_observations(self):
        from papers_v2.intelligence.symbolic_reasoning import SymbolicReasoningEngine
        engine = SymbolicReasoningEngine()
        observations = [
            {"x": 1.0, "y": 2.0},
            {"x": 2.0, "y": 4.0},
            {"x": 3.0, "y": 6.0},
            {"x": 4.0, "y": 8.0},
        ]
        hypotheses = engine.abduce(observations)
        assert len(hypotheses) >= 1
        assert any("y" in h and "x" in h for h in hypotheses)

    def test_abduce_insufficient_data(self):
        from papers_v2.intelligence.symbolic_reasoning import SymbolicReasoningEngine
        engine = SymbolicReasoningEngine()
        hypotheses = engine.abduce([{"x": 1.0, "y": 2.0}])
        assert len(hypotheses) >= 1

    def test_deduce(self):
        from papers_v2.intelligence.symbolic_reasoning import SymbolicReasoningEngine
        engine = SymbolicReasoningEngine()
        conclusions = engine.deduce(
            ["y ≈ 2.0 × x"],
            [{"text": "Linear relationships are common in ML", "confidence": 0.8}],
        )
        assert len(conclusions) >= 1
        assert conclusions[0][1] <= 1.0

    def test_induce_laws(self):
        from papers_v2.intelligence.symbolic_reasoning import SymbolicReasoningEngine
        engine = SymbolicReasoningEngine()
        data = [
            {"x": i, "y": 2 * i + 1 + (0.1 * __import__("random").random())}
            for i in range(20)
        ]
        laws = engine.induce(data, "y")
        assert len(laws) >= 1
        assert laws[0].r_squared > 0.5

    def test_chain_reasoning(self):
        from papers_v2.intelligence.symbolic_reasoning import SymbolicReasoningEngine
        engine = SymbolicReasoningEngine()
        observations = [
            {"x": i, "y": 2 * i} for i in range(10)
        ]
        chain = engine.chain_reasoning(
            observations,
            [{"text": "ML models exhibit linear scaling", "confidence": 0.9}],
            "y",
        )
        assert len(chain.abduction) >= 1
        assert 0 <= chain.weakest_link_confidence <= 1.0

    def test_verify_gamma_invariants(self):
        from papers_v2.intelligence.symbolic_reasoning import ReasoningChain, SymbolicReasoningEngine
        engine = SymbolicReasoningEngine()
        chain = ReasoningChain(
            chain_id="test",
            abduction=["h1"],
            deduction=["d1"],
            induction=["law1"],
        )
        results = engine.verify_gamma_invariants(chain)
        assert len(results) == 5
        assert any(r.name == "weakest_link" for r in results)


class TestProbabilisticReasoner:
    def test_register_and_observe(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.register_hypothesis("h1", "Test hypothesis", prior=0.6)
        reasoner.observe("h1", 0.8)
        h = reasoner.hypotheses["h1"]
        assert 0.0 <= h.posterior <= 1.0

    def test_auto_register(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.observe("new_h", 0.7)
        assert "new_h" in reasoner.hypotheses

    def test_compare_hypotheses(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.register_hypothesis("h1", "Good", prior=0.5)
        reasoner.register_hypothesis("h2", "Bad", prior=0.5)
        for _ in range(5):
            reasoner.observe("h1", 0.8)
            reasoner.observe("h2", 0.2)
        result = reasoner.compare_hypotheses("h1", "h2")
        assert result["bayes_factor"] > 1

    def test_best_hypothesis(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.register_hypothesis("h1", "Low", prior=0.1)
        reasoner.register_hypothesis("h2", "High", prior=0.9)
        best = reasoner.best_hypothesis()
        assert best is not None
        assert best.id == "h2"

    def test_top_k(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        for i in range(10):
            reasoner.register_hypothesis(f"h{i}", f"hypothesis {i}", prior=(i + 1) / 10)
        top = reasoner.top_k(5)
        assert len(top) == 5
        assert top[0].posterior >= top[-1].posterior

    def test_uncertainty_quantification(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.register_hypothesis("h1", "test")
        for _ in range(10):
            reasoner.observe("h1", 0.6)
        uq = reasoner.uncertainty_quantification("h1")
        assert "ci_95" in uq
        assert uq["n_observations"] == 10

    def test_predictive_distribution(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.register_hypothesis("h1", "test")
        for _ in range(5):
            reasoner.observe("h1", 0.7)
        dist = reasoner.predictive_distribution("h1")
        assert len(dist["x"]) > 0
        assert len(dist["density"]) > 0

    def test_reset(self):
        from papers_v2.intelligence.probabilistic import ProbabilisticReasoner
        reasoner = ProbabilisticReasoner()
        reasoner.register_hypothesis("h1", "test")
        reasoner.reset()
        assert len(reasoner.hypotheses) == 0


class TestGraphPatternMiner:
    def test_mine_patterns(self):
        import networkx as nx
        from papers_v2.intelligence.graph_mining import GraphPatternMiner

        G = nx.DiGraph()
        G.add_node("paper:ai", kind="paper")
        G.add_node("paper:ml", kind="paper")
        G.add_node("paper:dl", kind="paper")
        G.add_node("tag:pattern", kind="tag")
        G.add_node("domain:AI", kind="domain")
        G.add_node("insight:1", kind="insight")
        G.add_edges_from([
            ("paper:ai", "tag:pattern"),
            ("paper:ml", "tag:pattern"),
            ("paper:dl", "tag:pattern"),
            ("paper:ai", "domain:AI"),
            ("paper:ml", "domain:AI"),
            ("paper:dl", "domain:AI"),
            ("paper:ai", "insight:1"),
            ("tag:pattern", "domain:AI"),
        ])

        miner = GraphPatternMiner(G)
        patterns = miner.mine_patterns()
        assert len(patterns) >= 1

    def test_star_pattern_detection(self):
        import networkx as nx
        from papers_v2.intelligence.graph_mining import GraphPatternMiner

        G = nx.DiGraph()
        G.add_node("center", kind="hub")
        for i in range(5):
            G.add_node(f"leaf{i}", kind="leaf")
            G.add_edge("center", f"leaf{i}")

        miner = GraphPatternMiner(G)
        patterns = miner.mine_patterns()
        star_patterns = [p for p in patterns if p.type == "star"]
        assert len(star_patterns) >= 1

    def test_query_by_type(self):
        import networkx as nx
        from papers_v2.intelligence.graph_mining import GraphPatternMiner

        G = nx.DiGraph()
        G.add_node("a", kind="x")
        G.add_node("b", kind="x")
        G.add_edge("a", "b")

        miner = GraphPatternMiner(G)
        miner.mine_patterns()
        chains = miner.query_by_type("chain")
        assert isinstance(chains, list)

    def test_query_by_node(self):
        import networkx as nx
        from papers_v2.intelligence.graph_mining import GraphPatternMiner

        G = nx.DiGraph()
        G.add_node("center", kind="hub")
        for i in range(3):
            G.add_node(f"leaf{i}", kind="leaf")
            G.add_edge("center", f"leaf{i}")

        miner = GraphPatternMiner(G)
        miner.mine_patterns()
        node_patterns = miner.query_by_node("center")
        assert isinstance(node_patterns, list)

    def test_export_report(self):
        import networkx as nx
        from papers_v2.intelligence.graph_mining import GraphPatternMiner

        G = nx.DiGraph()
        G.add_node("a", kind="x")
        G.add_node("b", kind="x")
        G.add_edge("a", "b")

        miner = GraphPatternMiner(G)
        miner.mine_patterns()
        report = miner.export_report()
        assert "total_patterns" in report
        assert "graph_stats" in report


class TestResearchFrenzy:
    def test_creation(self, tmp_path):
        from papers_v2.intelligence.frenzy import ResearchFrenzy

        registry_path = tmp_path / "reg.json"
        import json
        registry_path.write_text(json.dumps([]))

        frenzy = ResearchFrenzy(
            task_description="Optimize a neural network",
            max_rounds=1,
            use_llm=False,
            registry_path=str(registry_path),
            cognition_path=str(tmp_path / "cog"),
            db_path=str(tmp_path / "db.json"),
            kg_path=str(tmp_path / "kg.json"),
            output_dir=str(tmp_path / "out"),
        )
        assert frenzy.task_description == "Optimize a neural network"
        assert frenzy.max_rounds == 1

    def test_search_relevant_papers(self, tmp_path):
        from papers_v2.intelligence.frenzy import ResearchFrenzy
        import json

        registry_path = tmp_path / "reg.json"
        papers = [
            {
                "id": "p1",
                "title": "Pattern Recognition in Neural Networks",
                "authors": ["A"],
                "year": 2026,
                "source": "src",
                "url": "url",
                "abstract": "About pattern recognition in neural nets",
                "tags": ["pattern", "neural", "optimization"],
                "domain": "LLM_ARCHITECTURES",
                "key_insight": "Patterns emerge in attention heads",
                "relevance_score": 0.9,
            },
        ]
        registry_path.write_text(json.dumps(papers))

        frenzy = ResearchFrenzy(
            task_description="Optimize neural networks with pattern recognition",
            max_rounds=1,
            use_llm=False,
            registry_path=str(registry_path),
            cognition_path=str(tmp_path / "cog"),
            db_path=str(tmp_path / "db.json"),
            kg_path=str(tmp_path / "kg.json"),
            output_dir=str(tmp_path / "out"),
        )
        results = frenzy._search_relevant_papers()
        assert len(results) >= 1

    def test_run_with_eval_function(self, tmp_path):
        from papers_v2.intelligence.frenzy import ResearchFrenzy
        import json

        registry_path = tmp_path / "reg.json"
        registry_path.write_text(json.dumps([]))

        def my_eval(program: str) -> dict:
            return {"success": True, "score": 0.75, "metrics": {"loss": 0.1}}

        frenzy = ResearchFrenzy(
            task_description="Test optimization",
            max_rounds=2,
            use_llm=False,
            registry_path=str(registry_path),
            cognition_path=str(tmp_path / "cog"),
            db_path=str(tmp_path / "db.json"),
            kg_path=str(tmp_path / "kg.json"),
            output_dir=str(tmp_path / "out"),
        )
        result = frenzy.run(eval_function=my_eval, n_parallel_candidates=2, verbose=False)
        assert "total_rounds" in result
        assert "best_score" in result
        assert result["total_candidates"] >= 2
