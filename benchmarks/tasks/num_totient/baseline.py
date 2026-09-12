

def totient(n: int) -> int:
    remaining: int = n
    count: int = n
    factor: int = 2
    while factor * factor <= remaining:
        if remaining % factor == 0:
            count = count - count // factor
            while remaining % factor == 0:
                remaining = remaining // factor
        factor = factor + 1
    if remaining > 1:
        count = count - count // remaining
    return count
