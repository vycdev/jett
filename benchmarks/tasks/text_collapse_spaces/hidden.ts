import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve(" "), "", 1);
check(solve("   "), "", 2);
check(solve("a"), "a", 3);
check(solve(" a "), "a", 4);
check(solve("a  b"), "a b", 5);
check(solve(" a  b c "), "a b c", 6);
check(solve("a\tb"), "a\tb", 7);
check(solve(" \t "), "\t", 8);
check(solve("a\n  b"), "a\n b", 9);
check(solve("!  ?"), "! ?", 10);
check(solve(" x   x  x "), "x x x", 11);
