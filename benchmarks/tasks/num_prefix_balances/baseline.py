

def nth(values: list[int], index: int) -> int:
    return values[index]

def prefix_balances(values: list[int], opening: int) -> list[int]:
    balances: list[int] = [opening]
    balance: int = opening
    index: int = 0
    while index < len(values):
        balance = balance + nth(values, index)
        balances.append(balance)
        index = index + 1
    return balances
