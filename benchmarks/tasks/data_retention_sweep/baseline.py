from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    key: int
    modified: int
    size: int

@dataclass(frozen=True)
class Report:
    removed: int
    reclaimed: int
    retained: int

def newest_index(rows: list[Entry], key: int) -> int:
    best_index: int = (-1)
    best_time: int = (-1000000)
    index: int = 0
    for row in rows:
        if ((row.key == key) and (row.modified >= best_time)):
            best_index = index
            best_time = row.modified
        index = (index + 1)
    return best_index

def solve(rows: list[Entry], limit: int) -> Report:
    removed: int = 0
    reclaimed: int = 0
    retained: int = 0
    index: int = 0
    for row in rows:
        if ((row.modified < limit) and (index != newest_index(rows, row.key))):
            removed = (removed + 1)
            reclaimed = (reclaimed + row.size)
        else:
            retained = (retained + 1)
        index = (index + 1)
    return Report(removed, reclaimed, retained)
