

def nth(values: list[int], index: int) -> int:
    return values[index]

def unique_sorted(values: list[int]) -> list[int]:
    output: list[int] = []
    index: int = 0
    while index < len(values):
        value: int = nth(values, index)
        if index == 0:
            output.append(value)
        else:
            if value != nth(values, index - 1):
                output.append(value)
        index = index + 1
    return output
