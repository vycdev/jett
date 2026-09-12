from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    patient: int
    severity: int
    arrival: int

@dataclass(frozen=True)
class Report:
    selected: int
    patient_checksum: int
    severity_sum: int

def precedes(severity: int, arrival: int, index: int, other_severity: int, other_arrival: int, other_index: int) -> bool:
    if (other_severity != severity):
        return (other_severity > severity)
    if (other_arrival != arrival):
        return (other_arrival < arrival)
    return (other_index < index)

def rank_of(rows: list[Entry], severity: int, arrival: int, index: int) -> int:
    rank: int = 1
    other_index: int = 0
    for other in rows:
        if precedes(severity, arrival, index, other.severity, other.arrival, other_index):
            rank = (rank + 1)
        other_index = (other_index + 1)
    return rank

def solve(rows: list[Entry], limit: int) -> Report:
    selected: int = 0
    checksum: int = 0
    total: int = 0
    index: int = 0
    for row in rows:
        rank: int = rank_of(rows, row.severity, row.arrival, index)
        if (rank <= limit):
            selected = (selected + 1)
            checksum = (checksum + (rank * (row.patient + 1)))
            total = (total + row.severity)
        index = (index + 1)
    return Report(selected, checksum, total)
