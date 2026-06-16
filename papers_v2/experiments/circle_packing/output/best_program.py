import math, random

def place_circles(n):
    """Hexagonal packing of n circles in unit square."""
    cols = int(math.ceil(math.sqrt(n * 2 / math.sqrt(3))))
    r = 1.0 / (2 * cols)
    row_h = r * math.sqrt(3)
    result = []
    placed = 0
    row = 0
    while placed < n:
        n_cols = cols if row % 2 == 0 else cols - 1
        for col in range(n_cols):
            if placed >= n:
                break
            x = r * (1 + 2 * col) if row % 2 == 0 else r * (2 + 2 * col)
            y = r + row * row_h
            if y + r <= 1.0 and x - r >= 0 and x + r <= 1.0:
                result.append((x, y, r))
                placed += 1
        row += 1
    return result[:n]