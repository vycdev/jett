from solution import solve

assert solve("", "") == 1, 0
assert solve("", "a") == 0, 1
assert solve("abc", "") == 4, 2
assert solve("aaaa", "aa") == 3, 3
assert solve("ababa", "aba") == 2, 4
assert solve("abc", "abc") == 1, 5
assert solve("ab", "abc") == 0, 6
assert solve("AaA", "a") == 1, 7
assert solve("xxxxx", "xx") == 4, 8
assert solve("one one", "one") == 2, 9
assert solve(" ", " ") == 1, 10
assert solve("abc", "z") == 0, 11
