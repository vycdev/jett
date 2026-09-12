from solution import solve

assert solve("/") == "/", 0
assert solve("///") == "/", 1
assert solve("/a/b") == "/a/b", 2
assert solve("/a//b/") == "/a/b", 3
assert solve("/./a/.") == "/a", 4
assert solve("/a/../b") == "/b", 5
assert solve("/../../a") == "/a", 6
assert solve("/a/b/../../") == "/", 7
assert solve("/.../..") == "/", 8
assert solve("/a/...") == "/a/...", 9
assert solve("/a/ /b") == "/a/ /b", 10
assert solve("/a/../a/./b/..") == "/a", 11
