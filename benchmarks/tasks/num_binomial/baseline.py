

def binomial(n: int, k: int) -> int:
    if k > n:
        return 0
    answer: int = 1
    index: int = 1
    while index <= k:
        answer = answer * (n - index + 1) // index
        index = index + 1
    return answer
