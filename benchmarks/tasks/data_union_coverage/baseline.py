from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    start: int
    end: int

@dataclass(frozen=True)
class Report:
    covered: int
    gaps: int
    longest_gap: int

def covered_at(rows: list[Entry], moment: int) -> bool:
    for row in rows:
        if ((row.start <= moment) and (row.end > moment)):
            return True
    return False

def solve(rows: list[Entry], limit: int) -> Report:
    covered: int = 0
    gaps: int = 0
    longest: int = 0
    current: int = 0
    moment: int = 0
    while (moment < limit):
        if covered_at(rows, moment):
            covered = (covered + 1)
            current = 0
        else:
            if (current == 0):
                gaps = (gaps + 1)
            current = (current + 1)
            if (current > longest):
                longest = current
        moment = (moment + 1)
    return Report(covered, gaps, longest)
