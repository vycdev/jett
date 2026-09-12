

def integer_sqrt(n: int) -> int:
    lower: int = 0
    upper: int = 1000001
    while lower + 1 < upper:
        middle: int = (lower + upper) // 2
        if middle * middle <= n:
            lower = middle
        else:
            upper = middle
    return lower
