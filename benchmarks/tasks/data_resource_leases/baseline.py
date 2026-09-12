from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    resource_id: int
    timestamp: int
    duration: int

@dataclass(frozen=True)
class Report:
    accepted: int
    rejected: int
    expires_sum: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry]) -> Report:
    expiries: dict[int, int] = {}
    accepted: int = 0
    rejected: int = 0
    total: int = 0
    for row in rows:
        prior: int = expiries.get(row.resource_id, 0)
        if ((row.timestamp >= prior) and (row.duration > 0)):
            next_expiry: int = (row.timestamp + row.duration)
            total = ((total + next_expiry) - prior)
            updated_value_0: int = next_expiry
            expiries = put(expiries, row.resource_id, updated_value_0)
            accepted = (accepted + 1)
        else:
            rejected = (rejected + 1)
    return Report(accepted, rejected, total)
