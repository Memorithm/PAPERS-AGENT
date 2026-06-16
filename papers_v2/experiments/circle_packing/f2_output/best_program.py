import math
import random

def place_circles(n):
    """
    Places n circles in a unit square [0,1] x [0,1] using a greedy placement strategy
    to maximize the minimum radius.
    """
    if n <= 0:
        return []

    # Initialize list to store circle placements (x, y, r)
    circles = []

    # The unit square boundaries
    x_min, y_min = 0.0, 0.0
    x_max, y_max = 1.0, 1.0

    # --- Greedy Placement Strategy ---
    for i in range(n):
        best_x, best_y, best_r = -1.0, -1.0, -1.0
        max_min_dist = -1.0

        # Try placing the circle at random positions within the square
        # We use a limited number of attempts for efficiency (as per requirement)
        num_attempts = 500
        found_placement = False

        for _ in range(num_attempts):
            # Randomly sample center coordinates (x, y) within the square
            x = random.uniform(0.0, 1.0)
            y = random.uniform(0.0, 1.0)

            # Calculate the minimum distance from this potential center to all boundaries and existing circles
            min_dist_to_boundary = min(x, 1.0 - x, y, 1.0 - y)

            min_dist_to_circles = float('inf')
            for cx, cy, cr in circles:
                # Distance to the center of existing circles
                dist = math.sqrt((x - cx)**2 + (y - cy)**2)
                # The minimum clearance required is the distance minus the radius of the existing circle
                min_dist_to_circles = min(min_dist_to_circles, dist - cr)

            # The resulting radius r is limited by the tightest constraint (boundary or other circles)
            current_r = min(min_dist_to_boundary, min_dist_to_circles)

            if current_r > max_min_dist:
                max_min_dist = current_r
                best_x, best_y, best_r = x, y, current_r
                found_placement = True

        # If a valid placement was found, add it to the list
        if found_placement:
            circles.append((best_x, best_y, best_r))
        else:
            # Fallback: If random search fails (highly unlikely), place at center with minimal radius
            # This ensures progress even if optimization stalls.
            center_x = 0.5
            center_y = 0.5
            min_dist_to_boundary = 0.5
            min_dist_to_circles = float('inf')
            for cx, cy, cr in circles:
                dist = math.sqrt((center_x - cx)**2 + (center_y - cy)**2)
                min_dist_to_circles = min(min_dist_to_circles, dist - cr)
            
            fallback_r = min(min_dist_to_boundary, min_dist_to_circles)
            if fallback_r > 0:
                 circles.append((center_x, center_y, fallback_r))

    return circles

# Example Usage (for testing purposes, not part of the required function definition):
# n_circles = 10
# result = place_circles(n_circles)
# print(f