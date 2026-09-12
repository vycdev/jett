

def nth(values: list[int], index: int) -> int:
    return values[index]

def max_product_pair(values: list[int]) -> int | None:
    if len(values) < 2:
        return None
    best: int = nth(values, 0) * nth(values, 1)
    first: int = 0
    while first < len(values):
        second: int = first + 1
        while second < len(values):
            candidate: int = nth(values, first) * nth(values, second)
            if candidate > best:
                best = candidate
            second = second + 1
        first = first + 1
    return best
