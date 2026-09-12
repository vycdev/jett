from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    party: int
    seats: int

@dataclass(frozen=True)
class Report:
    accepted: int
    rejected: int
    occupied: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry], limit: int) -> Report:
    parties: dict[int, int] = {}
    accepted: int = 0
    rejected: int = 0
    occupied: int = 0
    for row in rows:
        invalid: bool = ((row.seats <= 0) or (parties.get(row.party, 0) == 1))
        if (invalid or ((occupied + row.seats) > limit)):
            rejected = (rejected + 1)
        else:
            accepted = (accepted + 1)
            occupied = (occupied + row.seats)
            updated_value_0: int = 1
            parties = put(parties, row.party, updated_value_0)
    return Report(accepted, rejected, occupied)
