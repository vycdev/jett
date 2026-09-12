

def nth(values: list[int], index: int) -> int:
    return values[index]

def has_step(blocked: list[int], step: int) -> bool:
    index: int = 0
    while index < len(blocked):
        if nth(blocked, index) == step:
            return True
        index = index + 1
    return False

def staircase_blocked(height: int, blocked: list[int]) -> int:
    previous: int = 0
    current: int = 1
    step: int = 1
    while step <= height:
        following: int = current + previous
        if has_step(blocked, step):
            following = 0
        previous = current
        current = following
        step = step + 1
    return current
