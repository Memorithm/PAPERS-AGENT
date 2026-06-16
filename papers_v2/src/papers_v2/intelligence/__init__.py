"""
Intelligence modules - PAPER-driven autonomous code generation
Based on 34 papers on pattern recognition, deduction, and AI self-improvement.
"""

from papers_v2.intelligence.pattern_induction import PatternInductionEngine, Pattern
from papers_v2.intelligence.falsification import FalsificationEngine, FalsificationTest, FalsificationResult
from papers_v2.intelligence.verifier import CounterexampleGuidedVerifier, VerificationReport
from papers_v2.intelligence.symbolic_reasoning import (
    SymbolicReasoningEngine, SymbolicLaw, ReasoningChain, AlgebraicInvariant,
)
from papers_v2.intelligence.probabilistic import ProbabilisticReasoner, Hypothesis
from papers_v2.intelligence.graph_mining import GraphPatternMiner, GraphPattern

__all__ = [
    "AlgebraicInvariant",
    "CounterexampleGuidedVerifier",
    "FalsificationEngine",
    "FalsificationResult",
    "FalsificationTest",
    "GraphPattern",
    "GraphPatternMiner",
    "Hypothesis",
    "Pattern",
    "PatternInductionEngine",
    "ProbabilisticReasoner",
    "ReasoningChain",
    "SymbolicLaw",
    "SymbolicReasoningEngine",
    "VerificationReport",
]
