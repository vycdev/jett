from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    start: int
    end: int
    demand: int

@dataclass(frozen=True)
class Report:
    peak: int
    earliest: int
    overloaded_starts: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def load_at(rows: list[Entry], moment: int) -> int:
    total: int = 0
    for row in rows:
        if ((row.start <= moment) and (row.end > moment)):
            total = (total + row.demand)
    return total

def solve(rows: list[Entry], limit: int) -> Report:
    seen: dict[int, int] = {}
    peak: int = 0
    earliest: int = (-1)
    overloaded: int = 0
    for row in rows:
        load: int = load_at(rows, row.start)
        if ((load > peak) or ((load == peak) and (row.start < earliest))):
            peak = load
            earliest = row.start
        if (seen.get(row.start, 0) == 0):
            if (load > limit):
                overloaded = (overloaded + 1)
            updated_value_0: int = 1
            seen = put(seen, row.start, updated_value_0)
    return Report(peak, earliest, overloaded)
