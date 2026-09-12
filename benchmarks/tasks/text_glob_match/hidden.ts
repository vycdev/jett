import { solve } from "./solution.js";
function check(got: boolean, want: boolean, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve("", ""), true, 0);
check(solve("", "*"), true, 1);
check(solve("", "?"), false, 2);
check(solve("abc", "a?c"), true, 3);
check(solve("abc", "a*c"), true, 4);
check(solve("abbbc", "a*b*c"), true, 5);
check(solve("abc", "*d"), false, 6);
check(solve("abcd", "a*d?"), false, 7);
check(solve("abcdef", "*?c*f"), true, 8);
check(solve("aaaab", "a*b"), true, 9);
check(solve("abc", "**a**?**c**"), true, 10);
check(solve("ab", "a"), false, 11);
check(solve("a.b", "a.b"), true, 12);
check(solve("ABC", "abc"), false, 13);
