

def nth(values: list[int], index: int) -> int:
    return values[index]

def pair_sum_count(values: list[int], target: int) -> int:
    count: int = 0
    first: int = 0
    while first < len(values):
        second: int = first + 1
        while second < len(values):
            if nth(values, first) + nth(values, second) == target:
                count = count + 1
            second = second + 1
        first = first + 1
    return count
