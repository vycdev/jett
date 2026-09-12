from solution import solve

assert solve("1", "1") == 0, 0
assert solve("1", "1.0.0") == 0, 1
assert solve("0.0", "0") == 0, 2
assert solve("1.2", "1.10") == -1, 3
assert solve("2", "1.999") == 1, 4
assert solve("1.0.1", "1") == 1, 5
assert solve("1", "1.0.1") == -1, 6
assert solve("999", "998.999") == 1, 7
assert solve("0.1", "0.0.9") == 1, 8
assert solve("1.2.3", "1.2.3") == 0, 9
assert solve("1.2.3", "1.2.4") == -1, 10
assert solve("1.0.0.0.0.0.0.0", "1") == 0, 11
