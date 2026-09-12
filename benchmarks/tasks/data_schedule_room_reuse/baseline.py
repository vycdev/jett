from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    start: int
    end: int

@dataclass(frozen=True)
class Report:
    rooms: int
    assignment_checksum: int
    reuses: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry]) -> Report:
    ends: dict[int, int] = {}
    rooms: int = 0
    checksum: int = 0
    reuses: int = 0
    index: int = 1
    for row in rows:
        selected: int = 0
        room: int = 1
        while (room <= rooms):
            if ((selected == 0) and (ends.get(room, 0) <= row.start)):
                selected = room
            room = (room + 1)
        if (selected == 0):
            rooms = (rooms + 1)
            selected = rooms
        else:
            reuses = (reuses + 1)
        updated_value_0: int = row.end
        ends = put(ends, selected, updated_value_0)
        checksum = (checksum + (index * selected))
        index = (index + 1)
    return Report(rooms, checksum, reuses)
