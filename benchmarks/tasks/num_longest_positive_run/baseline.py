

def nth(values: list[int], index: int) -> int:
    return values[index]

def longest_positive_run(values: list[int]) -> int:
    current: int = 0
    best: int = 0
    index: int = 0
    while index < len(values):
        if nth(values, index) > 0:
            current = current + 1
            if current > best:
                best = current
        else:
            current = 0
        index = index + 1
    return best
