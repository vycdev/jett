import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve("", ""), "", 0);
check(solve("a", ""), "", 1);
check(solve("", "a"), "", 2);
check(solve("abc", "abc"), "abc", 3);
check(solve("abc", "abd"), "ab", 4);
check(solve("abc", "ab"), "ab", 5);
check(solve("ab", "abc"), "ab", 6);
check(solve("a", "A"), "", 7);
check(solve("foo bar", "foo baz"), "foo ba", 8);
check(solve(" x", " y"), " ", 9);
check(solve("123", "129"), "12", 10);
check(solve("abc", "zabc"), "", 11);
