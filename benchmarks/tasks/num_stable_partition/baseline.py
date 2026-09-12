

def nth(values: list[int], index: int) -> int:
    return values[index]

def stable_partition(values: list[int]) -> list[int]:
    output: list[int] = []
    index: int = 0
    while index < len(values):
        value: int = nth(values, index)
        if value < 0:
            output.append(value)
        index = index + 1
    index = 0
    while index < len(values):
        value: int = nth(values, index)
        if value >= 0:
            output.append(value)
        index = index + 1
    return output
