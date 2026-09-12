import { solve } from "./solution.js";
function check(got: number, want: number, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve("", ""), 1, 0);
check(solve("", "a"), 0, 1);
check(solve("abc", ""), 4, 2);
check(solve("aaaa", "aa"), 3, 3);
check(solve("ababa", "aba"), 2, 4);
check(solve("abc", "abc"), 1, 5);
check(solve("ab", "abc"), 0, 6);
check(solve("AaA", "a"), 1, 7);
check(solve("xxxxx", "xx"), 4, 8);
check(solve("one one", "one"), 2, 9);
check(solve(" ", " "), 1, 10);
check(solve("abc", "z"), 0, 11);
