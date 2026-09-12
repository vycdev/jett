from solution import solve

assert solve("") == "", 0
assert solve("abc") == "abc", 1
assert solve("#x") == "", 2
assert solve("a#b") == "a", 3
assert solve("a#b\nc") == "a\nc", 4
assert solve("\"#x\"#y") == "\"#x\"", 5
assert solve("#a\n#b\n") == "\n\n", 6
assert solve("a # x") == "a ", 7
assert solve("\"a\\\"#b\"#c") == "\"a\\\"#b\"", 8
assert solve("\\#x") == "\\", 9
assert solve("\"#open") == "\"#open", 10
assert solve("a\n#x\nb") == "a\n\nb", 11
