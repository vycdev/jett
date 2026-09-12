from solution import solve

assert solve("", "") == True, 0
assert solve("", "*") == True, 1
assert solve("", "?") == False, 2
assert solve("abc", "a?c") == True, 3
assert solve("abc", "a*c") == True, 4
assert solve("abbbc", "a*b*c") == True, 5
assert solve("abc", "*d") == False, 6
assert solve("abcd", "a*d?") == False, 7
assert solve("abcdef", "*?c*f") == True, 8
assert solve("aaaab", "a*b") == True, 9
assert solve("abc", "**a**?**c**") == True, 10
assert solve("ab", "a") == False, 11
assert solve("a.b", "a.b") == True, 12
assert solve("ABC", "abc") == False, 13
