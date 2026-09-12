from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    stock: int
    daily_demand: int

@dataclass(frozen=True)
class Report:
    restock_units: int
    at_risk: int
    worst_shortfall: int

def solve(rows: list[Entry], limit: int) -> Report:
    units: int = 0
    risk: int = 0
    worst: int = 0
    for row in rows:
        shortage: int = ((row.daily_demand * limit) - row.stock)
        if (shortage > 0):
            units = (units + shortage)
            risk = (risk + 1)
            if (shortage > worst):
                worst = shortage
    return Report(units, risk, worst)
