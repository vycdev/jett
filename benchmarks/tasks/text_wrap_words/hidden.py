from solution import solve

assert solve("", 5) == "", 0
assert solve("   ", 3) == "", 1
assert solve("a b c", 3) == "a b\nc", 2
assert solve("a b c", 1) == "a\nb\nc", 3
assert solve("longword x", 3) == "longword\nx", 4
assert solve("one two three", 7) == "one two\nthree", 5
assert solve(" one  two ", 80) == "one two", 6
assert solve("ab cd ef", 5) == "ab cd\nef", 7
assert solve("abc d", 3) == "abc\nd", 8
assert solve("a bc", 4) == "a bc", 9
assert solve("a bc", 3) == "a\nbc", 10
assert solve("x longword y", 3) == "x\nlongword\ny", 11
