

def nth(values: list[int], index: int) -> int:
    return values[index]

def polynomial_value(coefficients: list[int], x: int) -> int:
    answer: int = 0
    index: int = len(coefficients)
    while index > 0:
        index = index - 1
        answer = answer * x + nth(coefficients, index)
    return answer
