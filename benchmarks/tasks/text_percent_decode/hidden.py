from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextAccepted(""), 0
assert solve("abc") == TextAccepted("abc"), 1
assert solve("a+b") == TextAccepted("a+b"), 2
assert solve("%41%20%7e") == TextAccepted("A ~"), 3
assert solve("%25") == TextAccepted("%"), 4
assert solve("%") == TextRejected(TextError.MALFORMED), 5
assert solve("%2") == TextRejected(TextError.MALFORMED), 6
assert solve("%GG") == TextRejected(TextError.MALFORMED), 7
assert solve("%00") == TextRejected(TextError.RANGE), 8
assert solve("%7F") == TextRejected(TextError.RANGE), 9
assert solve("%ff") == TextRejected(TextError.RANGE), 10
assert solve("%00%G0") == TextRejected(TextError.RANGE), 11
assert solve("%G0%00") == TextRejected(TextError.MALFORMED), 12
assert solve("x%2fy") == TextAccepted("x/y"), 13
