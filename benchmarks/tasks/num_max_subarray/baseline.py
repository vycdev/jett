

def nth(values: list[int], index: int) -> int:
    return values[index]

def max_subarray(values: list[int]) -> int | None:
    if len(values) == 0:
        return None
    ending: int = nth(values, 0)
    best: int = ending
    index: int = 1
    while index < len(values):
        value: int = nth(values, index)
        if ending > 0:
            ending = ending + value
        else:
            ending = value
        if ending > best:
            best = ending
        index = index + 1
    return best
