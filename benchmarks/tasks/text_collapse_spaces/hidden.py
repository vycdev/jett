from solution import solve

assert solve("") == "", 0
assert solve(" ") == "", 1
assert solve("   ") == "", 2
assert solve("a") == "a", 3
assert solve(" a ") == "a", 4
assert solve("a  b") == "a b", 5
assert solve(" a  b c ") == "a b c", 6
assert solve("a\tb") == "a\tb", 7
assert solve(" \t ") == "\t", 8
assert solve("a\n  b") == "a\n b", 9
assert solve("!  ?") == "! ?", 10
assert solve(" x   x  x ") == "x x x", 11
