import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve("abc"), "abc", 1);
check(solve("&"), "&amp;", 2);
check(solve("<>"), "&lt;&gt;", 3);
check(solve("\"'"), "&quot;&#39;", 4);
check(solve("&amp;"), "&amp;amp;", 5);
check(solve("a&b<c"), "a&amp;b&lt;c", 6);
check(solve("&&"), "&amp;&amp;", 7);
check(solve("\n\t"), "\n\t", 8);
check(solve("/a=1"), "/a=1", 9);
check(solve("<a x=\"b\">"), "&lt;a x=&quot;b&quot;&gt;", 10);
check(solve("'&'"), "&#39;&amp;&#39;", 11);
