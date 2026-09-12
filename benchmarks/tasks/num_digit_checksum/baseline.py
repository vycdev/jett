

def digit_checksum(n: int) -> int:
    remaining: int = n
    sign: int = 1
    checksum: int = 0
    while remaining > 0:
        checksum = checksum + sign * (remaining % 10)
        sign = -sign
        remaining = remaining // 10
    return checksum
