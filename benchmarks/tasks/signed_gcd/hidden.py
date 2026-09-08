from solution import signed_gcd

assert signed_gcd(0, 0) == 0
assert signed_gcd(54, 24) == 6
assert signed_gcd(-54, 24) == 6
assert signed_gcd(54, -24) == 6
assert signed_gcd(-7, -13) == 1
assert signed_gcd(0, 42) == 42
assert signed_gcd(270, 192) == 6
