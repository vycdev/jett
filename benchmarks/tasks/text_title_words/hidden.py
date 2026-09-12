from solution import solve

assert solve("") == "", 0
assert solve(" ") == " ", 1
assert solve("HELLO") == "Hello", 2
assert solve("hello WORLD") == "Hello World", 3
assert solve("  a  B ") == "  A  B ", 4
assert solve("a-b") == "A-b", 5
assert solve("1ABC") == "1abc", 6
assert solve("!HELLO") == "!hello", 7
assert solve("x\tY") == "X\ty", 8
assert solve("a b c") == "A B C", 9
assert solve("MiXeD") == "Mixed", 10
assert solve("A  B  C") == "A  B  C", 11
