from solution import solve, TextAccepted, TextRejected, TextError

assert solve("") == TextRejected(TextError.EMPTY), 0
assert solve("  ") == TextRejected(TextError.EMPTY), 1
assert solve("a") == TextAccepted("a"), 2
assert solve("a bb c") == TextAccepted("bb"), 3
assert solve("ab cd") == TextAccepted("ab"), 4
assert solve(" xxx yy ") == TextAccepted("xxx"), 5
assert solve("a\tb zz") == TextAccepted("a\tb"), 6
assert solve("! ??") == TextAccepted("??"), 7
assert solve("long short") == TextAccepted("short"), 8
assert solve("same same") == TextAccepted("same"), 9
assert solve("   end") == TextAccepted("end"), 10
assert solve("ab abc abcd") == TextAccepted("abcd"), 11
