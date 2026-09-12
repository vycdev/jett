from solution import solve

assert solve("") == "", 0
assert solve(" ") == "", 1
assert solve("a") == "a", 2
assert solve("a b") == "b a", 3
assert solve(" a  b c ") == "c b a", 4
assert solve("one two three") == "three two one", 5
assert solve("a a b") == "b a a", 6
assert solve("! ?") == "? !", 7
assert solve("a\tb c") == "c a\tb", 8
assert solve("word  ") == "word", 9
assert solve("  x") == "x", 10
assert solve("ab cd ef gh") == "gh ef cd ab", 11
