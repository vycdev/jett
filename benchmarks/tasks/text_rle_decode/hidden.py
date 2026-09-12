from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextAccepted(""), 0
assert solve("1A") == TextAccepted("A"), 1
assert solve("3A2B") == TextAccepted("AAABB"), 2
assert solve("1A1A") == TextAccepted("AA"), 3
assert solve("100Z") == TextAccepted("ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"), 4
assert solve("0A") == TextRejected(TextError.MALFORMED), 5
assert solve("01A") == TextRejected(TextError.MALFORMED), 6
assert solve("101A") == TextRejected(TextError.MALFORMED), 7
assert solve("2a") == TextRejected(TextError.MALFORMED), 8
assert solve("A") == TextRejected(TextError.MALFORMED), 9
assert solve("12") == TextRejected(TextError.MALFORMED), 10
assert solve("60A41B") == TextRejected(TextError.RANGE), 11
assert solve("2A0B") == TextRejected(TextError.MALFORMED), 12
