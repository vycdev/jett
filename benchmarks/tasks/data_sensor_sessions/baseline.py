from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    sensor: int
    operation: int
    timestamp: int

@dataclass(frozen=True)
class Report:
    completed: int
    active: int
    rejected: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def step(operation: int, timestamp: int, opened: int) -> int:
    if (timestamp < 0):
        return (-2)
    if ((operation == 0) and (opened == (-1))):
        return timestamp
    if ((operation == 1) and (opened >= 0) and (timestamp >= opened)):
        return (-1)
    return (-2)

def solve(rows: list[Entry]) -> Report:
    opened: dict[int, int] = {}
    completed: int = 0
    active: int = 0
    rejected: int = 0
    for row in rows:
        prior: int = opened.get(row.sensor, (-1))
        next_time: int = step(row.operation, row.timestamp, prior)
        if (next_time == (-2)):
            rejected = (rejected + 1)
        else:
            updated_value_0: int = next_time
            opened = put(opened, row.sensor, updated_value_0)
            if (next_time == (-1)):
                completed = (completed + 1)
                active = (active - 1)
            else:
                active = (active + 1)
    return Report(completed, active, rejected)
