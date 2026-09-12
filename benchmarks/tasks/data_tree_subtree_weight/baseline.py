from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    node: int
    parent: int
    weight: int

@dataclass(frozen=True)
class Report:
    node_count: int
    total_weight: int
    leaf_count: int

def belongs(rows: list[Entry], node: int, selected: int) -> bool:
    current: int = node
    while (current != 0):
        if (current == selected):
            return True
        parent: int = 0
        for row in rows:
            if (row.node == current):
                parent = row.parent
        current = parent
    return False

def is_leaf(rows: list[Entry], node: int) -> bool:
    for row in rows:
        if (row.parent == node):
            return False
    return True

def solve(rows: list[Entry], limit: int) -> Report:
    count: int = 0
    total: int = 0
    leaves: int = 0
    for row in rows:
        if belongs(rows, row.node, limit):
            count = (count + 1)
            total = (total + row.weight)
            if is_leaf(rows, row.node):
                leaves = (leaves + 1)
    return Report(count, total, leaves)
