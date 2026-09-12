

def modular_power(base: int, exponent: int, modulus: int) -> int:
    power: int = base % modulus
    remaining: int = exponent
    answer: int = 1 % modulus
    while remaining > 0:
        if remaining % 2 == 1:
            answer = answer * power % modulus
        power = power * power % modulus
        remaining = remaining // 2
    return answer
