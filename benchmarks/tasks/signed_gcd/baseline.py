def signed_gcd(a: int, b: int) -> int:
    left, right = abs(a), abs(b)
    while right != 0:
        left, right = right, left % right
    return left
