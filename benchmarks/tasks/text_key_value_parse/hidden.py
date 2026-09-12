from solution import solve, TextPair

assert solve("a=b") == TextPair("a", "b"), 0
assert solve(" a = b ") == TextPair("a", "b"), 1
assert solve("a=") == TextPair("a", ""), 2
assert solve("a=  ") == TextPair("a", ""), 3
assert solve("a=b=c") == TextPair("a", "b=c"), 4
assert solve("a==") == TextPair("a", "="), 5
assert solve("x y=z z") == TextPair("x y", "z z"), 6
assert solve("\ta\t=\tb\t") == TextPair("\ta\t", "\tb\t"), 7
assert solve("x= a  b ") == TextPair("x", "a  b"), 8
assert solve("KEY=value") == TextPair("KEY", "value"), 9
assert solve("a= = ") == TextPair("a", "="), 10
assert solve("  k  = v=x ") == TextPair("k", "v=x"), 11
