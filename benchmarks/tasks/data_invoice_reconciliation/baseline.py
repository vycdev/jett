from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    invoice: int
    amount: int

@dataclass(frozen=True)
class Report:
    settled: int
    outstanding: int
    credit: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry]) -> Report:
    sums: dict[int, int] = {}
    for row in rows:
        updated_value_0: int = (sums.get(row.invoice, 0) + row.amount)
        sums = put(sums, row.invoice, updated_value_0)
    seen: dict[int, int] = {}
    settled: int = 0
    outstanding: int = 0
    credit: int = 0
    for row in rows:
        if (seen.get(row.invoice, 0) == 0):
            updated_value_1: int = 1
            seen = put(seen, row.invoice, updated_value_1)
            balance: int = sums.get(row.invoice, 0)
            if (balance == 0):
                settled = (settled + 1)
            if (balance > 0):
                outstanding = (outstanding + balance)
            if (balance < 0):
                credit = (credit - balance)
    return Report(settled, outstanding, credit)
