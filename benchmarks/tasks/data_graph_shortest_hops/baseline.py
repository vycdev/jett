from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    source: int
    target: int

@dataclass(frozen=True)
class Report:
    distance_sum: int
    farthest: int
    unreachable: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def relax(rows: list[Entry], distance: dict[int, int]) -> dict[int, int]:
    updated: dict[int, int] = {}
    for row in rows:
        prior: int = distance.get(row.source, 1000)
        old: int = distance.get(row.target, 1000)
        best: int = updated.get(row.target, old)
        if ((prior + 1) < best):
            updated_value_0: int = (prior + 1)
            updated = put(updated, row.target, updated_value_0)
    return updated

def solve(rows: list[Entry], limit: int) -> Report:
    distance: dict[int, int] = {}
    updated_value_1: int = 0
    distance = put(distance, 0, updated_value_1)
    turn: int = 0
    while (turn < limit):
        updated: dict[int, int] = relax(rows, distance)
        vertex: int = 0
        while (vertex < limit):
            updated_value_2: int = updated.get(vertex, distance.get(vertex, 1000))
            distance = put(distance, vertex, updated_value_2)
            vertex = (vertex + 1)
        turn = (turn + 1)
    total: int = 0
    farthest: int = 0
    missing: int = 0
    vertex: int = 0
    while (vertex < limit):
        hops: int = distance.get(vertex, 1000)
        if (hops == 1000):
            missing = (missing + 1)
        else:
            total = (total + hops)
            if (hops > farthest):
                farthest = hops
        vertex = (vertex + 1)
    return Report(total, farthest, missing)
