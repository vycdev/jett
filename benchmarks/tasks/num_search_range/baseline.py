from dataclasses import dataclass

@dataclass(frozen=True)
class SearchRange:
    first: int
    last: int

def nth(values: list[int], index: int) -> int:
    return values[index]

def search_range(values: list[int], target: int) -> SearchRange:
    first: int = -1
    last: int = -1
    index: int = 0
    while index < len(values):
        if nth(values, index) == target:
            if first == -1:
                first = index
            last = index
        index = index + 1
    return SearchRange(first, last)
