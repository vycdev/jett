from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextAccepted(""), 0
assert solve("41") == TextAccepted("A"), 1
assert solve("6869") == TextAccepted("hi"), 2
assert solve("207E") == TextAccepted(" ~"), 3
assert solve("5c22") == TextAccepted("\\\""), 4
assert solve("0") == TextRejected(TextError.MALFORMED), 5
assert solve("GG") == TextRejected(TextError.MALFORMED), 6
assert solve("00") == TextRejected(TextError.RANGE), 7
assert solve("7f") == TextRejected(TextError.RANGE), 8
assert solve("ff") == TextRejected(TextError.RANGE), 9
assert solve("00g") == TextRejected(TextError.MALFORMED), 10
assert solve("002G") == TextRejected(TextError.RANGE), 11
assert solve("2G00") == TextRejected(TextError.MALFORMED), 12
