from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    principal: int
    allowed: int
    specificity: int

@dataclass(frozen=True)
class Report:
    allowed: int
    denied: int
    defaulted: int

def solve(rows: list[Entry], limit: int) -> Report:
    allowed: int = 0
    denied: int = 0
    defaulted: int = 0
    principal: int = 0
    while (principal < limit):
        priority: int = (-1)
        decision: int = 0
        for row in rows:
            if ((row.principal == principal) and (row.specificity >= priority)):
                priority = row.specificity
                decision = row.allowed
        if (priority == (-1)):
            defaulted = (defaulted + 1)
        allowed = (allowed + decision)
        denied = ((denied + 1) - decision)
        principal = (principal + 1)
    return Report(allowed, denied, defaulted)
