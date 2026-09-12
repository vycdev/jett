from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    source: int
    target: int
    cost: int

@dataclass(frozen=True)
class Report:
    reachable: int
    cost_sum: int
    most_expensive: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry], limit: int) -> Report:
    costs: dict[int, int] = {}
    updated_value_0: int = 0
    costs = put(costs, 0, updated_value_0)
    reachable: int = 0
    total: int = 0
    maximum: int = 0
    vertex: int = 0
    while (vertex < limit):
        best: int = costs.get(vertex, 1000000)
        for row in rows:
            if (row.target == vertex):
                candidate: int = (costs.get(row.source, 1000000) + row.cost)
                if (candidate < best):
                    best = candidate
        updated_value_1: int = best
        costs = put(costs, vertex, updated_value_1)
        if (best < 1000000):
            reachable = (reachable + 1)
            total = (total + best)
            if (best > maximum):
                maximum = best
        vertex = (vertex + 1)
    return Report(reachable, total, maximum)
