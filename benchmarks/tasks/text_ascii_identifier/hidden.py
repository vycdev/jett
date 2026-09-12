from solution import solve

assert solve("") == False, 0
assert solve("_") == True, 1
assert solve("a") == True, 2
assert solve("A0_z") == True, 3
assert solve("0a") == False, 4
assert solve("a-b") == False, 5
assert solve("a b") == False, 6
assert solve("if") == True, 7
assert solve("__") == True, 8
assert solve("a\n") == False, 9
assert solve("$a") == False, 10
assert solve("z9") == True, 11
