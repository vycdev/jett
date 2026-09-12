

def collatz_steps(n: int, limit: int) -> int | None:
    current: int = n
    steps: int = 0
    while current != 1:
        if steps == limit:
            return None
        if current % 2 == 0:
            current = current // 2
        else:
            current = 3 * current + 1
        steps = steps + 1
    return steps
