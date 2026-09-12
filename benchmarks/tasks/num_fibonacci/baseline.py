

def fibonacci(n: int) -> int:
    previous: int = 0
    current: int = 1
    index: int = 0
    while index < n:
        following: int = previous + current
        previous = current
        current = following
        index = index + 1
    return previous
