from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    source: int
    target: int

@dataclass(frozen=True)
class Report:
    reachable: int
    unreachable: int
    reachable_edge_count: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry], limit: int) -> Report:
    seen: dict[int, int] = {}
    if (limit > 0):
        updated_value_0: int = 1
        seen = put(seen, 0, updated_value_0)
    turn: int = 0
    while (turn < limit):
        for row in rows:
            if (seen.get(row.source, 0) == 1):
                updated_value_1: int = 1
                seen = put(seen, row.target, updated_value_1)
        turn = (turn + 1)
    reachable: int = 0
    vertex: int = 0
    while (vertex < limit):
        reachable = (reachable + seen.get(vertex, 0))
        vertex = (vertex + 1)
    edges: int = 0
    for row in rows:
        edges = (edges + seen.get(row.source, 0))
    return Report(reachable, (limit - reachable), edges)
