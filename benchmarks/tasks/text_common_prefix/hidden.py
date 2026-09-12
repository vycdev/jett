from solution import solve

assert solve("", "") == "", 0
assert solve("a", "") == "", 1
assert solve("", "a") == "", 2
assert solve("abc", "abc") == "abc", 3
assert solve("abc", "abd") == "ab", 4
assert solve("abc", "ab") == "ab", 5
assert solve("ab", "abc") == "ab", 6
assert solve("a", "A") == "", 7
assert solve("foo bar", "foo baz") == "foo ba", 8
assert solve(" x", " y") == " ", 9
assert solve("123", "129") == "12", 10
assert solve("abc", "zabc") == "", 11
