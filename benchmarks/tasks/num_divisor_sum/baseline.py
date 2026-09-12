

def divisor_sum(n: int) -> int:
    total: int = 0
    divisor: int = 1
    while divisor * divisor <= n:
        if n % divisor == 0:
            total = total + divisor
            partner: int = n // divisor
            if partner != divisor:
                total = total + partner
        divisor = divisor + 1
    return total
