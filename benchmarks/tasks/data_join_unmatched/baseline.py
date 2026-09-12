from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    key: int
    side: int
    quantity: int

@dataclass(frozen=True)
class Report:
    matched: int
    left_only: int
    right_only: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry]) -> Report:
    left: dict[int, int] = {}
    right: dict[int, int] = {}
    for row in rows:
        if (row.side == 0):
            updated_value_0: int = (left.get(row.key, 0) + row.quantity)
            left = put(left, row.key, updated_value_0)
        if (row.side == 1):
            updated_value_1: int = (right.get(row.key, 0) + row.quantity)
            right = put(right, row.key, updated_value_1)
    seen: dict[int, int] = {}
    matched: int = 0
    left_only: int = 0
    right_only: int = 0
    for row in rows:
        if (seen.get(row.key, 0) == 0):
            updated_value_2: int = 1
            seen = put(seen, row.key, updated_value_2)
            a: int = left.get(row.key, 0)
            b: int = right.get(row.key, 0)
            pairs: int = a
            if (b < pairs):
                pairs = b
            matched = (matched + pairs)
            left_only = ((left_only + a) - pairs)
            right_only = ((right_only + b) - pairs)
    return Report(matched, left_only, right_only)
