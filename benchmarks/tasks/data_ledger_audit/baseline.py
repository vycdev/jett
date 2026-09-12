from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    account: int
    delta: int

@dataclass(frozen=True)
class Report:
    total: int
    lowest_balance: int
    first_overdraw: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry], limit: int) -> Report:
    balances: dict[int, int] = {}
    seen: dict[int, int] = {}
    total: int = 0
    lowest: int = limit
    first: int = (-1)
    index: int = 0
    for row in rows:
        if (seen.get(row.account, 0) == 0):
            total = (total + limit)
            updated_value_0: int = 1
            seen = put(seen, row.account, updated_value_0)
        balance: int = (balances.get(row.account, limit) + row.delta)
        updated_value_1: int = balance
        balances = put(balances, row.account, updated_value_1)
        total = (total + row.delta)
        if ((index == 0) or (balance < lowest)):
            lowest = balance
        if ((balance < 0) and (first == (-1))):
            first = index
        index = (index + 1)
    return Report(total, lowest, first)
