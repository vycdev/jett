from solution import solve, TextAccepted, TextRejected, TextError

assert solve("00:00:00") == TextAccepted(0), 0
assert solve("23:59:59") == TextAccepted(86399), 1
assert solve("12:34:56") == TextAccepted(45296), 2
assert solve("01:02:03") == TextAccepted(3723), 3
assert solve("24:00:00") == TextRejected(TextError.RANGE), 4
assert solve("00:60:00") == TextRejected(TextError.RANGE), 5
assert solve("00:00:60") == TextRejected(TextError.RANGE), 6
assert solve("0:00:00") == TextRejected(TextError.MALFORMED), 7
assert solve("00-00-00") == TextRejected(TextError.MALFORMED), 8
assert solve("aa:00:00") == TextRejected(TextError.MALFORMED), 9
assert solve("99:xx:00") == TextRejected(TextError.MALFORMED), 10
assert solve("") == TextRejected(TextError.MALFORMED), 11
