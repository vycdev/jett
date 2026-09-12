from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextAccepted([""]), 0
assert solve(",") == TextAccepted(["", ""]), 1
assert solve("a,b,c") == TextAccepted(["a", "b", "c"]), 2
assert solve("\"a,b\",c") == TextAccepted(["a,b", "c"]), 3
assert solve("\"\"") == TextAccepted([""]), 4
assert solve("\"a\"\"b\",,") == TextAccepted(["a\"b", "", ""]), 5
assert solve("\"") == TextRejected(TextError.MALFORMED), 6
assert solve("a\"b") == TextRejected(TextError.MALFORMED), 7
assert solve("\"a\" x") == TextRejected(TextError.MALFORMED), 8
assert solve("\"a\",") == TextAccepted(["a", ""]), 9
assert solve("\"\"\"\"") == TextAccepted(["\""]), 10
assert solve("\"\"\"") == TextRejected(TextError.MALFORMED), 11
assert solve("a,b,,") == TextAccepted(["a", "b", "", ""]), 12
