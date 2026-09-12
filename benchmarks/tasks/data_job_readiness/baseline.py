from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    job: int
    prerequisite: int
    cost: int

@dataclass(frozen=True)
class Report:
    ready: int
    blocked: int
    total_cost: int

def ready_job(rows: list[Entry], job: int) -> bool:
    current: int = job
    while (current != 0):
        parent: int = (-1)
        for row in rows:
            if (row.job == current):
                parent = row.prerequisite
        if (parent == (-1)):
            return False
        current = parent
    return True

def solve(rows: list[Entry]) -> Report:
    ready: int = 0
    blocked: int = 0
    total: int = 0
    for row in rows:
        if ready_job(rows, row.job):
            ready = (ready + 1)
            total = (total + row.cost)
        else:
            blocked = (blocked + 1)
    return Report(ready, blocked, total)
