"""
Seed cognition store with circle packing heuristics.
"""

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../.."))

from papers_v2.evolution.cognition import CognitionBase, CognitionEntry

store = CognitionBase(persist_dir=os.path.join(os.path.dirname(__file__), "cognition_data"))

heuristics = [
    {
        "title": "Hexagonal Packing",
        "content": "Hexagonal close-packing achieves the highest density in 2D (pi/(2*sqrt(3)) ≈ 0.9069). "
                   "Circles should be arranged in a hexagonal lattice where each row is offset by half a circle width. "
                   "The optimal distance between centers is 2*r."
    },
    {
        "title": "Greedy Placement",
        "content": "Place circles one by one at the position that maximizes distance to existing circles "
                   "and boundaries. For each new circle, try random positions and pick the one with "
                   "the largest minimum distance to placed circles and walls."
    },
    {
        "title": "Simulated Annealing",
        "content": "Start with a valid configuration and iteratively perturb positions. "
                   "Accept moves that increase the minimum radius. Use temperature-based acceptance "
                   "for moves that decrease quality. Slowly cool to converge to optimum."
    },
    {
        "title": "Genetic Algorithm",
        "content": "Evolve a population of circle configurations. Crossover: combine positions from "
                   "two parents. Mutation: perturb positions slightly. Select top performers. "
                   "Fitness = minimum radius across all circles."
    },
    {
        "title": "Boundary Awareness",
        "content": "Circles near walls need special treatment. The effective distance to a wall is "
                   "twice the actual distance because the wall acts as a mirror. Place circles "
                   "at least r from each wall."
    },
    {
        "title": "Iterative Refinement",
        "content": "Start with a hexagonal grid, then iteratively: (1) find the smallest circle, "
                   "(2) slightly expand all circles, (3) resolve overlaps by pushing circles apart, "
                   "(4) repeat until convergence."
    },
    {
        "title": "Force-Directed Layout",
        "content": "Treat circles as physical objects with repulsive forces. Simulate physics: "
                   "circles repel each other and walls. Run until equilibrium. Then scale all radii "
                   "up until constraint violation."
    },
]

for item in heuristics:
    entry = CognitionEntry(
        content=item["content"],
        source=item["title"],
        entry_type="heuristic",
        tags=["circle_packing", "optimization", "geometry"],
    )
    store.add_entry(entry)

print(f"Seeded {len(heuristics)} cognition items")
