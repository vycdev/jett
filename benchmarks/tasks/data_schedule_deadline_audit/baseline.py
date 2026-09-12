from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    duration: int
    deadline: int
    penalty: int

@dataclass(frozen=True)
class Report:
    late: int
    weighted_tardiness: int
    finish: int

def solve(rows: list[Entry], limit: int) -> Report:
    late: int = 0
    weighted: int = 0
    finish: int = limit
    for row in rows:
        finish = (finish + row.duration)
        if (finish > row.deadline):
            late = (late + 1)
            weighted = (weighted + ((finish - row.deadline) * row.penalty))
    return Report(late, weighted, finish)
