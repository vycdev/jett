import { solve } from "./solution.js";
function check(got: boolean, want: boolean, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), false, 0);
check(solve("_"), true, 1);
check(solve("a"), true, 2);
check(solve("A0_z"), true, 3);
check(solve("0a"), false, 4);
check(solve("a-b"), false, 5);
check(solve("a b"), false, 6);
check(solve("if"), true, 7);
check(solve("__"), true, 8);
check(solve("a\n"), false, 9);
check(solve("$a"), false, 10);
check(solve("z9"), true, 11);
