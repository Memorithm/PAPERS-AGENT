"""
Circle Packing Experiment - ASI-Evolve / PAPERS V2
Pack N circles in a unit square, maximizing total area.
"""

import math, random, sys, json, importlib.util, os, signal

class TimeoutError(Exception):
    pass

def timeout_handler(signum, frame):
    raise TimeoutError("Evaluation timeout")

def load_program(path):
    spec = importlib.util.spec_from_file_location("candidate", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.place_circles

def evaluate(place_fn, n=26, trials=1):
    """Returns best score across multiple trials (stochastic algorithms)."""
    best_score = 0.0
    best_placement = None

    for _ in range(trials):
        try:
            placements = place_fn(n)
            if len(placements) != n:
                continue

            min_r = float('inf')
            valid = True
            for i, (x, y, r) in enumerate(placements):
                if r <= 0 or not (0 <= x <= 1 and 0 <= y <= 1):
                    valid = False
                    break
                # Check bounds
                if x - r < 0 or x + r > 1 or y - r < 0 or y + r > 1:
                    valid = False
                    break
                min_r = min(min_r, r)
                for j in range(i):
                    x2, y2, r2 = placements[j]
                    if math.sqrt((x-x2)**2 + (y-y2)**2) < r + r2 - 1e-10:
                        valid = False
                        break
                if not valid:
                    break

            if valid:
                score = min_r  # maximize minimum radius
                if score > best_score:
                    best_score = score
                    best_placement = placements
        except Exception:
            continue

    return best_score, best_placement

def baseline_grid(n):
    """Simple grid packing - baseline strategy."""
    cols = math.ceil(math.sqrt(n))
    rows = math.ceil(n / cols)
    cell_w = 1.0 / cols
    cell_h = 1.0 / rows
    r = min(cell_w, cell_h) / 2.0

    placements = []
    for i in range(n):
        col = i % cols
        row = i // cols
        x = cell_w * (col + 0.5)
        y = cell_h * (row + 0.5)
        placements.append((x, y, r))
    return placements

def baseline_hexagonal(n):
    """Hexagonal packing - better density."""
    cols = math.ceil(math.sqrt(n * 2 / math.sqrt(3)))
    r = 1.0 / (2 * cols)
    row_height = r * math.sqrt(3)

    placements = []
    placed = 0
    row = 0
    while placed < n:
        n_cols = cols if row % 2 == 0 else cols - 1
        for col in range(n_cols):
            if placed >= n:
                break
            x = r * (1 + 2 * col) if row % 2 == 0 else r * (2 + 2 * col)
            y = r + row * row_height
            if y + r <= 1.0 and x - r >= 0 and x + r <= 1.0:
                placements.append((x, y, r))
                placed += 1
        row += 1

    return placements[:n]

if __name__ == "__main__":
    signal.signal(signal.SIGALRM, timeout_handler)
    signal.alarm(10)  # 10 second hard timeout
    try:
        if len(sys.argv) < 2:
            for name, fn in [("grid", baseline_grid), ("hex", baseline_hexagonal)]:
                score, _ = evaluate(fn)
                print(json.dumps({"candidate": name, "score": score}))
        else:
            place_fn = load_program(sys.argv[1])
            score, placement = evaluate(place_fn)
            result = {"score": score, "success": score > 0, "metrics": {"n_circles": 26}}
            print(json.dumps(result))
    except TimeoutError:
        print(json.dumps({"score": 0.0, "success": False, "error": "timeout"}))
    except Exception as e:
        print(json.dumps({"score": 0.0, "success": False, "error": str(e)[:200]}))
    finally:
        signal.alarm(0)
