import { solve } from "./solution.js";
function check(got: boolean, want: boolean, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), true, 0);
check(solve("! ?"), true, 1);
check(solve("a"), true, 2);
check(solve("ab"), false, 3);
check(solve("Aa"), true, 4);
check(solve("Race car!"), true, 5);
check(solve("A man, a plan, a canal: Panama"), true, 6);
check(solve("12 21"), true, 7);
check(solve("12a21"), true, 8);
check(solve("12a22"), false, 9);
check(solve("0P"), false, 10);
check(solve("ab\tBA"), true, 11);
