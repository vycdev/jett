from solution import solve

assert solve("") == "", 0
assert solve("  ") == "", 1
assert solve("alice") == "A", 2
assert solve("alice bob") == "AB", 3
assert solve("  alice   bob ") == "AB", 4
assert solve("a b c") == "ABC", 5
assert solve("1 one") == "1O", 6
assert solve("! bang") == "!B", 7
assert solve("mixed CASE") == "MC", 8
assert solve("a\tb c") == "AC", 9
assert solve("z Z") == "ZZ", 10
assert solve("foo-bar baz") == "FB", 11
