

def nth(values: list[int], index: int) -> int:
    return values[index]

def window_peak(values: list[int], width: int) -> int | None:
    if width == 0:
        return None
    if width > len(values):
        return None
    total: int = 0
    index: int = 0
    while index < width:
        total = total + nth(values, index)
        index = index + 1
    best: int = total
    while index < len(values):
        total = total + nth(values, index) - nth(values, index - width)
        if total > best:
            best = total
        index = index + 1
    return best
