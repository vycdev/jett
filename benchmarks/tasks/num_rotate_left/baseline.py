

def nth(values: list[int], index: int) -> int:
    return values[index]

def rotate_left(values: list[int], distance: int) -> list[int]:
    rotated: list[int] = []
    count: int = len(values)
    if count == 0:
        return rotated
    shift: int = distance % count
    index: int = 0
    while index < count:
        position: int = (index + shift) % count
        rotated.append(nth(values, position))
        index = index + 1
    return rotated
