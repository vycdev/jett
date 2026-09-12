

def nth(values: list[int], index: int) -> int:
    return values[index]

def lis_length(values: list[int]) -> int:
    lengths: list[int] = []
    best: int = 0
    index: int = 0
    while index < len(values):
        ending: int = 1
        prior: int = 0
        while prior < index:
            if nth(values, prior) < nth(values, index):
                candidate: int = nth(lengths, prior) + 1
                if candidate > ending:
                    ending = candidate
            prior = prior + 1
        lengths.append(ending)
        if ending > best:
            best = ending
        index = index + 1
    return best
