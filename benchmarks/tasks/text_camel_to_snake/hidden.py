from solution import solve

assert solve("") == "", 0
assert solve("a") == "a", 1
assert solve("A") == "a", 2
assert solve("camelCase") == "camel_case", 3
assert solve("HTTPServer") == "http_server", 4
assert solve("XML") == "xml", 5
assert solve("parseURLValue") == "parse_url_value", 6
assert solve("x2Y") == "x2_y", 7
assert solve("ABc") == "a_bc", 8
assert solve("oneTwoThree") == "one_two_three", 9
assert solve("v123") == "v123", 10
assert solve("ABCdEF") == "ab_cd_ef", 11
