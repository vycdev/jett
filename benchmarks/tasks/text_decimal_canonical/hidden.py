from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextRejected(TextError.EMPTY), 0
assert solve("0") == TextAccepted("0"), 1
assert solve("-000") == TextAccepted("0"), 2
assert solve("+0012") == TextAccepted("12"), 3
assert solve("-012") == TextAccepted("-12"), 4
assert solve("123") == TextAccepted("123"), 5
assert solve("+") == TextRejected(TextError.MALFORMED), 6
assert solve("--1") == TextRejected(TextError.MALFORMED), 7
assert solve(" 1") == TextRejected(TextError.MALFORMED), 8
assert solve("1.0") == TextRejected(TextError.MALFORMED), 9
assert solve("0x0") == TextRejected(TextError.MALFORMED), 10
assert solve("9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999") == TextAccepted("9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999"), 11
assert solve("-0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001") == TextAccepted("-1"), 12
