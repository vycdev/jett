from dataclasses import dataclass

@dataclass(frozen=True)
class Mode:
    value: int
    count: int

def nth(values: list[int], index: int) -> int:
    return values[index]

def occurrence_count(values: list[int], target: int) -> int:
    count: int = 0
    index: int = 0
    while index < len(values):
        if nth(values, index) == target:
            count = count + 1
        index = index + 1
    return count

def histogram_mode(values: list[int]) -> Mode:
    best_value: int = 0
    best_count: int = 0
    index: int = 0
    while index < len(values):
        value: int = nth(values, index)
        count: int = occurrence_count(values, value)
        replace: bool = count > best_count
        if count == best_count:
            if value < best_value:
                replace = True
        if replace:
            best_value = value
            best_count = count
        index = index + 1
    return Mode(best_value, best_count)
