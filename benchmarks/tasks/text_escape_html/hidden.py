from solution import solve

assert solve("") == "", 0
assert solve("abc") == "abc", 1
assert solve("&") == "&amp;", 2
assert solve("<>") == "&lt;&gt;", 3
assert solve("\"'") == "&quot;&#39;", 4
assert solve("&amp;") == "&amp;amp;", 5
assert solve("a&b<c") == "a&amp;b&lt;c", 6
assert solve("&&") == "&amp;&amp;", 7
assert solve("\n\t") == "\n\t", 8
assert solve("/a=1") == "/a=1", 9
assert solve("<a x=\"b\">") == "&lt;a x=&quot;b&quot;&gt;", 10
assert solve("'&'") == "&#39;&amp;&#39;", 11
