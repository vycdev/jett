from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    passenger: int
    preferred: int

@dataclass(frozen=True)
class Report:
    assigned: int
    rejected: int
    seat_checksum: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def choose_seat(occupied: dict[int, int], preferred: int, limit: int) -> int:
    if ((preferred > 0) and (preferred <= limit)):
        if (occupied.get(preferred, 0) == 0):
            return preferred
    seat: int = 1
    while (seat <= limit):
        if (occupied.get(seat, 0) == 0):
            return seat
        seat = (seat + 1)
    return 0

def solve(rows: list[Entry], limit: int) -> Report:
    occupied: dict[int, int] = {}
    passengers: dict[int, int] = {}
    assigned: int = 0
    rejected: int = 0
    checksum: int = 0
    for row in rows:
        seat: int = 0
        if (passengers.get(row.passenger, 0) == 0):
            seat = choose_seat(occupied, row.preferred, limit)
        if (seat == 0):
            rejected = (rejected + 1)
        else:
            assigned = (assigned + 1)
            checksum = (checksum + ((row.passenger + 1) * seat))
            updated_value_0: int = 1
            occupied = put(occupied, seat, updated_value_0)
            updated_value_1: int = 1
            passengers = put(passengers, row.passenger, updated_value_1)
    return Report(assigned, rejected, checksum)
