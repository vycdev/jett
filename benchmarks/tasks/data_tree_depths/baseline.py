from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    node: int
    parent: int
    weight: int

@dataclass(frozen=True)
class Report:
    roots: int
    max_depth: int
    weighted_depth: int

def depth_of(rows: list[Entry], node: int) -> int:
    current: int = node
    depth: int = (-1)
    while (current != 0):
        parent: int = 0
        for row in rows:
            if (row.node == current):
                parent = row.parent
        current = parent
        depth = (depth + 1)
    return depth

def solve(rows: list[Entry]) -> Report:
    roots: int = 0
    maximum: int = 0
    total: int = 0
    for row in rows:
        depth: int = depth_of(rows, row.node)
        if (depth == 0):
            roots = (roots + 1)
        if (depth > maximum):
            maximum = depth
        total = (total + (row.weight * depth))
    return Report(roots, maximum, total)
