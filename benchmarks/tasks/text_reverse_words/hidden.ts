import { solve } from "./solution.js";
function check(got: string, want: string, index: number): void {
  if (got !== want) throw new Error(`case ${index}`);
}
check(solve(""), "", 0);
check(solve(" "), "", 1);
check(solve("a"), "a", 2);
check(solve("a b"), "b a", 3);
check(solve(" a  b c "), "c b a", 4);
check(solve("one two three"), "three two one", 5);
check(solve("a a b"), "b a a", 6);
check(solve("! ?"), "? !", 7);
check(solve("a\tb c"), "c a\tb", 8);
check(solve("word  "), "word", 9);
check(solve("  x"), "x", 10);
check(solve("ab cd ef gh"), "gh ef cd ab", 11);
