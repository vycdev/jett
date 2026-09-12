from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    event: int
    key: int
    delta: int

@dataclass(frozen=True)
class Report:
    applied: int
    duplicates: int
    checksum: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry]) -> Report:
    seen: dict[int, int] = {}
    applied: int = 0
    duplicates: int = 0
    checksum: int = 0
    for row in rows:
        if (seen.get(row.event, 0) == 0):
            updated_value_0: int = 1
            seen = put(seen, row.event, updated_value_0)
            checksum = (checksum + ((row.key + 1) * row.delta))
            applied = (applied + 1)
        else:
            duplicates = (duplicates + 1)
    return Report(applied, duplicates, checksum)
