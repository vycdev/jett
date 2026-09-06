def bounded_weighted_sum(values: list[int], cap: int) -> int:
    total = 0
    for index, value in enumerate(values):
        bounded = max(-cap, min(cap, value))
        total += bounded * (index + 1)
    return total
