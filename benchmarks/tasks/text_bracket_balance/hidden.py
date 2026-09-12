from solution import solve

assert solve("") == True, 0
assert solve("abc") == True, 1
assert solve("()[]{}") == True, 2
assert solve("([{}])") == True, 3
assert solve("([)]") == False, 4
assert solve("(") == False, 5
assert solve(")") == False, 6
assert solve("a[b(c)d]e") == True, 7
assert solve("{{}}") == True, 8
assert solve("{]") == False, 9
assert solve("\"(\"") == False, 10
assert solve("(()())") == True, 11
