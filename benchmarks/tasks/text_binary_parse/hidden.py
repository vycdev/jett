from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextRejected(TextError.EMPTY), 0
assert solve("0") == TextAccepted(0), 1
assert solve("1") == TextAccepted(1), 2
assert solve("00101") == TextAccepted(5), 3
assert solve("1111111111111111") == TextAccepted(65535), 4
assert solve("1000000000000000") == TextAccepted(32768), 5
assert solve("00000000000000000") == TextRejected(TextError.RANGE), 6
assert solve("2") == TextRejected(TextError.MALFORMED), 7
assert solve("10x") == TextRejected(TextError.MALFORMED), 8
assert solve(" 1") == TextRejected(TextError.MALFORMED), 9
assert solve("+1") == TextRejected(TextError.MALFORMED), 10
assert solve("101010") == TextAccepted(42), 11
assert solve("xxxxxxxxxxxxxxxxx") == TextRejected(TextError.RANGE), 12
