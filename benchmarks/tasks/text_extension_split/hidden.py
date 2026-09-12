from solution import solve, TextPair

assert solve("") == TextPair("", ""), 0
assert solve("a") == TextPair("a", ""), 1
assert solve("a.txt") == TextPair("a", "txt"), 2
assert solve("a.tar.gz") == TextPair("a.tar", "gz"), 3
assert solve(".env") == TextPair(".env", ""), 4
assert solve(".env.local") == TextPair(".env", "local"), 5
assert solve("a.") == TextPair("a", ""), 6
assert solve(".") == TextPair(".", ""), 7
assert solve("..") == TextPair(".", ""), 8
assert solve("...") == TextPair("..", ""), 9
assert solve("A.TXT") == TextPair("A", "TXT"), 10
assert solve(" a .x") == TextPair(" a ", "x"), 11
