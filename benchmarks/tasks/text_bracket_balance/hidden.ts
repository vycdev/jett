import { solve } from "./solution.js";
function check(got: boolean, want: boolean, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), true, 0);
check(solve("abc"), true, 1);
check(solve("()[]{}"), true, 2);
check(solve("([{}])"), true, 3);
check(solve("([)]"), false, 4);
check(solve("("), false, 5);
check(solve(")"), false, 6);
check(solve("a[b(c)d]e"), true, 7);
check(solve("{{}}"), true, 8);
check(solve("{]"), false, 9);
check(solve("\"(\""), false, 10);
check(solve("(()())"), true, 11);
