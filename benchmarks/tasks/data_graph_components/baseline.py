from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    source: int
    target: int

@dataclass(frozen=True)
class Report:
    components: int
    largest: int
    isolated: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def spread(rows: list[Entry], seen: dict[int, int], vertex: int) -> int:
    found: int = seen.get(vertex, 0)
    for row in rows:
        if (row.source == vertex):
            if (seen.get(row.target, 0) == 1):
                found = 1
        if (row.target == vertex):
            if (seen.get(row.source, 0) == 1):
                found = 1
    return found

def component_size(rows: list[Entry], start: int, limit: int) -> int:
    seen: dict[int, int] = {}
    updated_value_0: int = 1
    seen = put(seen, start, updated_value_0)
    turn: int = 0
    while (turn < limit):
        vertex: int = 0
        while (vertex < limit):
            updated_value_1: int = spread(rows, seen, vertex)
            seen = put(seen, vertex, updated_value_1)
            vertex = (vertex + 1)
        turn = (turn + 1)
    count: int = 0
    vertex: int = 0
    while (vertex < limit):
        if (seen.get(vertex, 0) == 1):
            if (vertex < start):
                return 0
            count = (count + 1)
        vertex = (vertex + 1)
    return count

def solve(rows: list[Entry], limit: int) -> Report:
    components: int = 0
    largest: int = 0
    isolated: int = 0
    vertex: int = 0
    while (vertex < limit):
        count: int = component_size(rows, vertex, limit)
        if (count > 0):
            components = (components + 1)
        if (count > largest):
            largest = count
        incident: int = 0
        for row in rows:
            if ((row.source == vertex) or (row.target == vertex)):
                incident = (incident + 1)
        if (incident == 0):
            isolated = (isolated + 1)
        vertex = (vertex + 1)
    return Report(components, largest, isolated)
