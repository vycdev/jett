

def nth(values: list[int], index: int) -> int:
    return values[index]

def sorted_insert(values: list[int], value: int) -> list[int]:
    output: list[int] = []
    inserted: bool = False
    index: int = 0
    while index < len(values):
        item: int = nth(values, index)
        if not inserted:
            if value <= item:
                output.append(value)
                inserted = True
        output.append(item)
        index = index + 1
    if not inserted:
        output.append(value)
    return output
