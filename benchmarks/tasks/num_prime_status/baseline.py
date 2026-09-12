

def prime_status(n: int) -> bool:
    if n < 2:
        return False
    divisor: int = 2
    while divisor * divisor <= n:
        if n % divisor == 0:
            return False
        divisor = divisor + 1
    return True
