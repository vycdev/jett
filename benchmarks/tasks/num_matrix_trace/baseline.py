

def nth(values: list[int], index: int) -> int:
    return values[index]

def matrix_trace(values: list[int], rows: int) -> int:
    total: int = 0
    row: int = 0
    while row < rows:
        total = total + nth(values, row * rows + row)
        row = row + 1
    return total
