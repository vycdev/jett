

def nth(values: list[int], index: int) -> int:
    return values[index]

def equilibrium_index(values: list[int]) -> int | None:
    remaining: int = 0
    index: int = 0
    while index < len(values):
        remaining = remaining + nth(values, index)
        index = index + 1
    before: int = 0
    index = 0
    while index < len(values):
        value: int = nth(values, index)
        remaining = remaining - value
        if before == remaining:
            return index
        before = before + value
        index = index + 1
    return None
