from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    node: int
    parent: int

@dataclass(frozen=True)
class Report:
    ancestor: int
    selected_distance: int
    largest_distance: int

def parent_of(rows: list[Entry], node: int) -> int:
    for row in rows:
        if (row.node == node):
            return row.parent
    return (-1)

def solve(rows: list[Entry], limit: int) -> Report:
    largest: int = 0
    for row in rows:
        if (row.node > largest):
            largest = row.node
    if (parent_of(rows, limit) == (-1)):
        return Report(0, (-1), (-1))
    selected: int = limit
    selected_distance: int = 0
    while (selected > 0):
        other: int = largest
        other_distance: int = 0
        while (other > 0):
            if (other == selected):
                return Report(selected, selected_distance, other_distance)
            other = parent_of(rows, other)
            other_distance = (other_distance + 1)
        selected = parent_of(rows, selected)
        selected_distance = (selected_distance + 1)
    return Report(0, (-1), (-1))
