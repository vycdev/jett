from solution import solve

assert solve("") == True, 0
assert solve("! ?") == True, 1
assert solve("a") == True, 2
assert solve("ab") == False, 3
assert solve("Aa") == True, 4
assert solve("Race car!") == True, 5
assert solve("A man, a plan, a canal: Panama") == True, 6
assert solve("12 21") == True, 7
assert solve("12a21") == True, 8
assert solve("12a22") == False, 9
assert solve("0P") == False, 10
assert solve("ab\tBA") == True, 11
