def first_duplicate(values: list[int]) -> int | None:
    seen: set[int] = set()
    for value in values:
        if value in seen:
            return value
        seen.add(value)
    return None
