from solution import solve

assert solve("") == "\"\"", 0
assert solve("a") == "\"a\"", 1
assert solve("\"") == "\"\\\"\"", 2
assert solve("\\") == "\"\\\\\"", 3
assert solve("\n") == "\"\\n\"", 4
assert solve("\t") == "\"\\t\"", 5
assert solve("\r") == "\"\\r\"", 6
assert solve("\b") == "\"\\b\"", 7
assert solve("\f") == "\"\\f\"", 8
assert solve("/") == "\"/\"", 9
assert solve("a\"b\\c") == "\"a\\\"b\\\\c\"", 10
assert solve("x\ny\tz") == "\"x\\ny\\tz\"", 11
