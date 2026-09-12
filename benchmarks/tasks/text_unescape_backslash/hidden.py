from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextAccepted(""), 0
assert solve("abc") == TextAccepted("abc"), 1
assert solve("\\n") == TextAccepted("\n"), 2
assert solve("\\t\\r") == TextAccepted("\t\r"), 3
assert solve("\\\\") == TextAccepted("\\"), 4
assert solve("\\\"") == TextAccepted("\""), 5
assert solve("a\\nb") == TextAccepted("a\nb"), 6
assert solve("\\") == TextRejected(TextError.MALFORMED), 7
assert solve("x\\q") == TextRejected(TextError.MALFORMED), 8
assert solve("\\0") == TextRejected(TextError.MALFORMED), 9
assert solve("\\\\n") == TextAccepted("\\n"), 10
assert solve("a\nb") == TextAccepted("a\nb"), 11
