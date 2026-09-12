from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    prerequisite: int
    dependent: int

@dataclass(frozen=True)
class Report:
    layers: int
    last_layer_count: int
    layer_sum: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry], limit: int) -> Report:
    levels: dict[int, int] = {}
    maximum: int = (-1)
    last_count: int = 0
    total: int = 0
    vertex: int = 0
    while (vertex < limit):
        level: int = 0
        for row in rows:
            if (row.dependent == vertex):
                prior: int = (levels.get(row.prerequisite, 0) + 1)
                if (prior > level):
                    level = prior
        updated_value_0: int = level
        levels = put(levels, vertex, updated_value_0)
        total = (total + level)
        if (level > maximum):
            maximum = level
            last_count = 0
        if (level == maximum):
            last_count = (last_count + 1)
        vertex = (vertex + 1)
    return Report((maximum + 1), last_count, total)
